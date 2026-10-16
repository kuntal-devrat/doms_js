//! Microtask queue and event-loop pumping.

use super::*;

impl Vm {
    pub fn enqueue_microtask(&mut self, callee: JsValue, args: Vec<JsValue>) {
        self.microtasks.push_back((callee, args));
    }

    pub fn drain_microtasks(&mut self) -> Result<(), VmError> {
        let mut count = 0;
        while let Some((callee, args)) = self.microtasks.pop_front() {
            count += 1;
            if count > 1000 {
                return Err(VmError::RuntimeError("Microtask queue limit exceeded".into()));
            }
            let _ = self.call(callee, JsValue::undefined(), &args);
        }
        Ok(())
    }

    pub fn step_event_loop(&mut self) -> Result<bool, VmError> {
        self.drain_microtasks()?;
        let now = Instant::now();
        let mut due_id = None;
        for (&id, entry) in &self.timers {
            if entry.due_time <= now {
                due_id = Some(id);
                break;
            }
        }
        if let Some(id) = due_id {
            if let Some(mut entry) = self.timers.remove(&id) {
                let _ = self.call(entry.callback, JsValue::undefined(), &entry.args);
                if let Some(interval) = entry.interval {
                    entry.due_time = Instant::now() + interval;
                    self.timers.insert(id, entry);
                }
                self.drain_microtasks()?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn run_event_loop(&mut self) -> Result<(), VmError> {
        let max_ticks = 100_000;
        let mut ticks = 0;
        while !self.timers.is_empty() || !self.microtasks.is_empty() {
            ticks += 1;
            if ticks > max_ticks {
                return Err(VmError::RuntimeError("Event loop max ticks exceeded".into()));
            }
            let worked = self.step_event_loop()?;
            if !worked && !self.timers.is_empty() {
                let next_due = self.timers.values().map(|t| t.due_time).min();
                if let Some(due) = next_due {
                    let now = Instant::now();
                    if due > now {
                        let wait = (due - now).min(Duration::from_millis(5));
                        std::thread::sleep(wait);
                    }
                }
            }
        }
        Ok(())
    }
}
