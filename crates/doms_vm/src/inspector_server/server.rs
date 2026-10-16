//! The `InspectorServer` TCP listener lifecycle.

use super::*;

pub struct InspectorServer {
    pub(super) addr: String,
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
