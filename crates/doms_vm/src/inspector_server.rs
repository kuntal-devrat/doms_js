//! Chrome DevTools Protocol (CDP) live inspection TCP/WebSocket server.
//!
//! Enables connecting Google Chrome, Edge, and VS Code directly to DOMS:
//! `chrome://inspect` -> targets appear dynamically and support live stepping!

use crate::debugger::CdpDispatcher;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct InspectorServer {
    addr: String,
    script_url: String,
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    dispatcher: Arc<Mutex<CdpDispatcher>>,
    listener_thread: Option<thread::JoinHandle<()>>,
}

impl InspectorServer {
    pub fn start(addr: &str, script_url: &str, pause_on_start: bool) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let bound_addr = listener.local_addr()?.to_string();
        let running = Arc::new(AtomicBool::new(true));
        let paused = Arc::new(AtomicBool::new(pause_on_start));
        let dispatcher = Arc::new(Mutex::new(CdpDispatcher::new()));

        let running_clone = Arc::clone(&running);
        let paused_clone = Arc::clone(&paused);
        let dispatcher_clone = Arc::clone(&dispatcher);
        let addr_for_thread = bound_addr.clone();
        let script_for_thread = script_url.to_string();

        let handle = thread::spawn(move || {
            let _ = listener.set_nonblocking(true);
            while running_clone.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let disp = Arc::clone(&dispatcher_clone);
                        let p = Arc::clone(&paused_clone);
                        let host = addr_for_thread.clone();
                        let script = script_for_thread.clone();
                        thread::spawn(move || {
                            handle_client(stream, disp, p, &host, &script);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            addr: bound_addr,
            script_url: script_url.to_string(),
            running,
            paused,
            dispatcher,
            listener_thread: Some(handle),
        })
    }

    pub fn script_url(&self) -> &str {
        &self.script_url
    }

    pub fn dispatcher(&self) -> Arc<Mutex<CdpDispatcher>> {
        Arc::clone(&self.dispatcher)
    }

    pub fn ws_url(&self) -> String {
        format!("ws://{}/devtools/page/doms", self.addr)
    }

    pub fn devtools_url(&self) -> String {
        format!(
            "devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws={}/devtools/page/doms",
            self.addr
        )
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn wait_for_resume(&self) {
        while self.is_paused() && self.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.listener_thread.take() {
            let _ = h.join();
        }
    }
}

impl Drop for InspectorServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn handle_client(
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
            "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=UTF-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{{\"Browser\":\"DOMS/0.1.0\",\"Protocol-Version\":\"1.3\"}}",
            44
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    // 2. JSON Discovery: GET /json or GET /json/list
    if req_str.starts_with("GET /json") {
        let body = format!(
            "[{{\"description\":\"DOMS.js V8/CDP target\",\"devtoolsFrontendUrl\":\"devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws={}/devtools/page/doms\",\"id\":\"doms-isolate-1\",\"title\":\"DOMS.js Runtime\",\"type\":\"node\",\"url\":\"{}\",\"webSocketDebuggerUrl\":\"ws://{}/devtools/page/doms\"}}]",
            host, script_url, host
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=UTF-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
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

// ── WebSocket Framing & Protocol (RFC 6455) ──

fn read_websocket_frame(stream: &mut TcpStream) -> std::io::Result<Option<String>> {
    let mut header = [0u8; 2];
    if stream.read_exact(&mut header).is_err() {
        return Ok(None);
    }

    let opcode = header[0] & 0x0F;
    if opcode == 8 {
        // Connection Close
        return Ok(None);
    }

    let is_masked = (header[1] & 0x80) != 0;
    let mut payload_len = (header[1] & 0x7F) as usize;

    if payload_len == 126 {
        let mut ext = [0u8; 2];
        stream.read_exact(&mut ext)?;
        payload_len = u16::from_be_bytes(ext) as usize;
    } else if payload_len == 127 {
        let mut ext = [0u8; 8];
        stream.read_exact(&mut ext)?;
        payload_len = u64::from_be_bytes(ext) as usize;
    }

    let mut mask_key = [0u8; 4];
    if is_masked {
        stream.read_exact(&mut mask_key)?;
    }

    let mut payload = vec![0u8; payload_len];
    stream.read_exact(&mut payload)?;

    if is_masked {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask_key[i % 4];
        }
    }

    Ok(Some(String::from_utf8_lossy(&payload).into_owned()))
}

fn write_websocket_frame(stream: &mut TcpStream, text: &str) -> std::io::Result<()> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut frame = Vec::with_capacity(len + 10);

    // FIN = 1, Opcode = 1 (Text)
    frame.push(0x81);

    if len <= 125 {
        frame.push(len as u8);
    } else if len <= 65535 {
        frame.push(126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    frame.extend_from_slice(bytes);
    stream.write_all(&frame)
}

// ── Pure SHA-1 & Base64 RFC 6455 Handshake ──

fn compute_websocket_accept(key: &str) -> String {
    let combined = format!("{}258EAFA5-E914-47DA-95CA-C5AB0DC85B11", key);
    let digest = sha1(combined.as_bytes());
    base64_encode(&digest)
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes(word.try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);

        for (i, &item) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(item);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };

        let idx0 = (b0 >> 2) as usize;
        let idx1 = (((b0 & 3) << 4) | (b1 >> 4)) as usize;
        let idx2 = (((b1 & 0x0F) << 2) | (b2 >> 6)) as usize;
        let idx3 = (b2 & 0x3F) as usize;

        out.push(TABLE[idx0] as char);
        out.push(TABLE[idx1] as char);
        if i + 1 < data.len() {
            out.push(TABLE[idx2] as char);
        } else {
            out.push('=');
        }
        if i + 2 < data.len() {
            out.push(TABLE[idx3] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_accept_computation() {
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = compute_websocket_accept(key);
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn test_inspector_server_discovery_and_lifecycle() {
        let server = InspectorServer::start("127.0.0.1:0", "test.js", false).expect("server start");
        assert!(server.ws_url().contains("ws://127.0.0.1:"));
        assert!(server.devtools_url().contains("devtools://"));

        // Query /json/version via HTTP
        let addr = server.addr.clone();
        let mut stream = TcpStream::connect(&addr).expect("connect");
        stream
            .write_all(b"GET /json/version HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("write");
        let mut buf = [0u8; 512];
        let n = stream.read(&mut buf).expect("read");
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("DOMS/0.1.0"));
    }
}
