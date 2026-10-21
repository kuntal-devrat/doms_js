//! Per-connection HTTP discovery and WebSocket upgrade handling.

use super::*;

pub(super) fn handle_client(
    mut stream: TcpStream,
    dispatcher: Arc<Mutex<CdpDispatcher>>,
    paused: Arc<AtomicBool>,
    host: &str,
    script_url: &str,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(5000)));
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let req_str = String::from_utf8_lossy(&buf[..n]);

    // 1. JSON Discovery: GET /json/version
    if req_str.starts_with("GET /json/version") {
        let resp = format!(
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json; charset=UTF-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{{\"Browser\":\"DOMS/0.1.0\",\"Protocol-Version\":\"1.3\"}}",
            44
        );
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.flush();
        let _ = stream.shutdown(std::net::Shutdown::Write);
        return;
    }

    // 2. JSON Discovery: GET /json or GET /json/list
    if req_str.starts_with("GET /json") {
        let body = format!(
            "[{{\"description\":\"DOMS.js V8/CDP target\",\"devtoolsFrontendUrl\":\"devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws={}/devtools/page/doms\",\"id\":\"doms-isolate-1\",\"title\":\"DOMS.js Runtime\",\"type\":\"node\",\"url\":\"{}\",\"webSocketDebuggerUrl\":\"ws://{}/devtools/page/doms\"}}]",
            host, script_url, host
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json; charset=UTF-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.flush();
        let _ = stream.shutdown(std::net::Shutdown::Write);
        return;
    }

    // 3. WebSocket Upgrade
    if req_str.contains("Upgrade: websocket") || req_str.contains("upgrade: websocket") {
        let key = extract_header(&req_str, "Sec-WebSocket-Key");
        if let Some(sec_key) = key {
            let accept_val = compute_websocket_accept(&sec_key);
            let handshake = format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                accept_val
            );
            if stream.write_all(handshake.as_bytes()).is_err() {
                return;
            }

            // Enter WebSocket bidirectional message loop
            let _ = stream.set_read_timeout(None);
            loop {
                match read_websocket_frame(&mut stream) {
                    Ok(Some(text)) => {
                        // Handle resume
                        if text.contains("Debugger.resume") {
                            paused.store(false, Ordering::SeqCst);
                        }
                        let mut disp = dispatcher.lock().unwrap();
                        if let Some(resp) = disp.dispatch(&text) {
                            if write_websocket_frame(&mut stream, &resp).is_err() {
                                break;
                            }
                        }
                    }
                    Ok(None) => break, // WebSocket closed
                    Err(_) => break,
                }
            }
        }
    }
}

fn extract_header(req: &str, header: &str) -> Option<String> {
    for line in req.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case(header) {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}
