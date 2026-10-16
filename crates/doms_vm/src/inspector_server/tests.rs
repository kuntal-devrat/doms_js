//! Tests for the inspector server and WebSocket handshake.

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
