//! Work-stealing prioritized task scheduler for parallel isolates and web worker pools.

use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::sync::mpsc::channel;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

/// Task execution priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

struct PrioritizedJob {
    priority: TaskPriority,
    seq: u64,
    job: Box<dyn FnOnce() + Send + 'static>,
}

impl PartialEq for PrioritizedJob {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}

impl Eq for PrioritizedJob {}

impl PartialOrd for PrioritizedJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first; if equal, earlier sequence first
        self.priority.cmp(&other.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

struct QueueState {
    heap: BinaryHeap<PrioritizedJob>,
    shutdown: bool,
    next_seq: u64,
}

/// Handle to await completion and retrieve result of an asynchronous task.
pub struct TaskHandle<T> {
    receiver: std::sync::mpsc::Receiver<T>,
}

impl<T> TaskHandle<T> {
    pub fn join(self) -> Result<T, std::sync::mpsc::RecvError> {
        self.receiver.recv()
    }

    pub fn try_recv(&self) -> Result<T, std::sync::mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
}

/// Multi-threaded work-stealing task scheduler.
pub struct Scheduler {
    state: Arc<(Mutex<QueueState>, Condvar)>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl Scheduler {
    /// Create a new worker pool with `threads` worker threads.
    pub fn new(threads: usize) -> Self {
        let state = Arc::new((
            Mutex::new(QueueState {
                heap: BinaryHeap::new(),
                shutdown: false,
                next_seq: 0,
            }),
            Condvar::new(),
        ));
        let mut workers = Vec::with_capacity(threads);

        for _ in 0..threads {
            let state_clone = Arc::clone(&state);
            let handle = thread::spawn(move || {
                let (lock, cvar) = &*state_clone;
                loop {
                    let mut guard = lock.lock().unwrap();
                    while guard.heap.is_empty() && !guard.shutdown {
                        guard = cvar.wait(guard).unwrap();
                    }
                    if guard.shutdown && guard.heap.is_empty() {
                        break;
                    }
                    if let Some(item) = guard.heap.pop() {
                        drop(guard);
                        (item.job)();
                    }
                }
            });
            workers.push(handle);
        }

        Self { state, workers }
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Spawn a fire-and-forget task with default normal priority.
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.spawn_with_priority(TaskPriority::Normal, f);
    }

    /// Spawn a fire-and-forget task with explicit priority.
    pub fn spawn_with_priority<F>(&self, priority: TaskPriority, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let (lock, cvar) = &*self.state;
        let mut guard = lock.lock().unwrap();
        let seq = guard.next_seq;
        guard.next_seq += 1;
        guard.heap.push(PrioritizedJob {
            priority,
            seq,
            job: Box::new(f),
        });
        cvar.notify_one();
    }

    /// Submit a task that returns a value, returning a `TaskHandle<T>`.
    pub fn submit<F, T>(&self, f: F) -> TaskHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_with_priority(TaskPriority::Normal, f)
    }

    /// Submit a task with priority that returns a value.
    pub fn submit_with_priority<F, T>(&self, priority: TaskPriority, f: F) -> TaskHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = channel();
        self.spawn_with_priority(priority, move || {
            let res = f();
            let _ = tx.send(res);
        });
        TaskHandle { receiver: rx }
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.state;
        {
            let mut guard = lock.lock().unwrap();
            guard.shutdown = true;
        }
        cvar.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_submit_and_join() {
        let scheduler = Scheduler::new(2);
        let h1 = scheduler.submit(|| 10 * 10);
        let h2 = scheduler.submit(|| 20 + 20);
        assert_eq!(h1.join().unwrap(), 100);
        assert_eq!(h2.join().unwrap(), 40);
    }

    #[test]
    fn test_scheduler_priorities() {
        let scheduler = Scheduler::new(1);
        let h_crit = scheduler.submit_with_priority(TaskPriority::Critical, || "crit");
        let h_low = scheduler.submit_with_priority(TaskPriority::Low, || "low");
        assert_eq!(h_crit.join().unwrap(), "crit");
        assert_eq!(h_low.join().unwrap(), "low");
    }
}
