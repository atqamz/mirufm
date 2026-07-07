use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::{Condvar, Mutex};
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    // Ordering: higher discriminant = higher priority (Visible runs first).
    Preload,
    Preview,
    Visible,
}

#[derive(Clone)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

pub struct TaskHandle {
    cancel: CancelFlag,
}

impl TaskHandle {
    pub fn cancel(&self) {
        self.cancel.0.store(true, Ordering::Relaxed);
    }
}

struct Job {
    priority: Priority,
    seq: u64,
    run: Box<dyn FnOnce(CancelFlag) + Send>,
    cancel: CancelFlag,
}

impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}
impl Eq for Job {}
impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Job {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority first; for equal priority, lower seq (FIFO) first.
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

struct Shared {
    queue: Mutex<(BinaryHeap<Job>, u64)>,
    ready: Condvar,
}

pub struct Scheduler {
    shared: Arc<Shared>,
}

impl Scheduler {
    pub fn new(workers: usize) -> Scheduler {
        let shared = Arc::new(Shared {
            queue: Mutex::new((BinaryHeap::new(), 0)),
            ready: Condvar::new(),
        });
        for _ in 0..workers.max(1) {
            let shared = Arc::clone(&shared);
            thread::spawn(move || loop {
                let job = {
                    let mut guard = shared.queue.lock().unwrap();
                    loop {
                        if let Some(job) = guard.0.pop() {
                            break job;
                        }
                        guard = shared.ready.wait(guard).unwrap();
                    }
                };
                (job.run)(job.cancel);
            });
        }
        Scheduler { shared }
    }

    pub fn spawn<F>(&self, priority: Priority, task: F) -> TaskHandle
    where
        F: FnOnce(CancelFlag) + Send + 'static,
    {
        let cancel = CancelFlag(Arc::new(AtomicBool::new(false)));
        let mut guard = self.shared.queue.lock().unwrap();
        let seq = guard.1;
        guard.1 += 1;
        guard.0.push(Job {
            priority,
            seq,
            run: Box::new(task),
            cancel: cancel.clone(),
        });
        drop(guard);
        self.shared.ready.notify_one();
        TaskHandle { cancel }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn runs_a_task_to_completion() {
        let sched = Scheduler::new(2);
        let (tx, rx) = mpsc::channel();
        sched.spawn(Priority::Visible, move |_cancel| {
            tx.send(42).unwrap();
        });
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), 42);
    }

    #[test]
    fn cancelled_task_observes_the_flag() {
        let sched = Scheduler::new(1);
        // Occupy the single worker so the next task queues.
        let (block_tx, block_rx) = mpsc::channel::<()>();
        sched.spawn(Priority::Visible, move |_cancel| {
            block_rx.recv().ok();
        });

        let (tx, rx) = mpsc::channel();
        let handle = sched.spawn(Priority::Visible, move |cancel| {
            tx.send(cancel.is_cancelled()).unwrap();
        });
        handle.cancel();
        block_tx.send(()).unwrap(); // release the worker

        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), true);
    }
}
