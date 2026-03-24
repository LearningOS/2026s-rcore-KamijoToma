//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A stride scheduler backed by a ready queue.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }

    /// Take the runnable process with the smallest stride pass.
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        let mut best: Option<(usize, usize, usize)> = None;
        for (idx, task) in self.ready_queue.iter().enumerate() {
            let pass = task.stride_pass();
            let pid = task.getpid();
            match best {
                Some((_, best_pass, best_pid)) if (pass, pid) >= (best_pass, best_pid) => {}
                _ => best = Some((idx, pass, pid)),
            }
        }
        let (idx, _, _) = best?;
        let task = self.ready_queue.remove(idx).unwrap();
        task.advance_stride();
        Some(task)
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
