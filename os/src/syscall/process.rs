//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, get_current_task_syscall_counter, suspend_current_and_run_next}, timer::get_time_us
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    // Each task has its own syscall counter in PCB, no need to reset
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

// TODO: implement the syscall
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request {
        // 0 stands for read a byte from an addr
        // 1 stands for write a byte to an addr
        // 2 stands for get the count of a syscall
        0 => unsafe { *(_id as *const u8) as isize },
        1 => {
            unsafe {
                let byte = _data as u8;
                *(_id as *mut u8) = byte;
            }
            0
        }
        2 => {
            let counter = get_current_task_syscall_counter();
            counter.get(&_id).copied().unwrap_or(0) as isize
        }
        _ => {
            trace!("kernel: sys_trace with invalid request {}", _trace_request);
            -1
        },
    }
}
