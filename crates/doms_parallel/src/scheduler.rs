//! Task scheduler for isolates and worker pools.

use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct Scheduler {
    sender: Sender<Job>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl Scheduler {
    pub fn new(threads: usize) -> Self {
        let (sender, receiver) = channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(threads);

        for _ in 0..threads {
            let rx = Arc::clone(&receiver);
            let handle = thread::spawn(move || {
                loop {
                    let job = {
                        let lock = rx.lock().unwrap();
                        lock.recv()
                    };
                    match job {
                        Ok(f) => f(),
                        Err(_) => break, // Channel closed
                    }
                }
            });
            workers.push(handle);
        }

        Self { sender, workers }
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let _ = self.sender.send(Box::new(f));
    }
}
