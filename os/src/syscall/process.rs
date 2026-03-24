//! Process management syscalls

use crate::{
    mm::{copy_data_to_user, read_user_byte, write_user_byte},
    task::{
        change_program_brk, current_task_syscall_counter, current_user_token,
        exit_current_and_run_next, mmap_current_task, munmap_current_task,
        suspend_current_and_run_next,
    },
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// Fill a userspace [`TimeVal`] with the current time in seconds and microseconds.
///
/// Returns `0` on success and `-1` if `ts` does not refer to a writable user buffer.
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let time_val = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    if copy_data_to_user(current_user_token(), ts, &time_val) {
        0
    } else {
        -1
    }
}

/// Inspect or modify user memory, or query the current task's syscall counters.
///
/// Supported requests are:
/// - `0`: read one byte from the user address `id`
/// - `1`: write one byte `data as u8` to the user address `id`
/// - `2`: query how many times syscall `id` has been invoked by the current task
///
/// Returns `-1` when the request is invalid or the target user address is not accessible
/// with the required permission.
pub fn sys_trace(_trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request {
        // 0 stands for read a byte from an addr
        // 1 stands for write a byte to an addr
        // 2 stands for get the count of a syscall
        0 => {
            match read_user_byte(current_user_token(), id as *const u8) {
                Some(value) => {
                    trace!("sys_trace read: addr = {:#x}, data = {:#x}", id, value);
                    value as isize
                }
                None => {
                    trace!("sys_trace read: invalid address {:#x}", id);
                    -1
                }
            }
        }
        1 => {
            trace!("sys_trace write: addr = {:#x}, data = {:#x}", id, data);
            if write_user_byte(current_user_token(), id as *mut u8, data as u8) {
                0
            } else {
                -1
            }
        }
        2 => {
            trace!("sys_trace count: id = {:#x}", id);
            let counter = current_task_syscall_counter();
            counter.get(&id).cloned().unwrap_or(0) as isize
        }
        _ => {
            trace!("sys_trace: invalid request {:#x}", _trace_request);
            -1
        }
    }
}

/// Create a user anonymous mapping in the current task's address space.
///
/// `start` must be page-aligned, `len` must be non-zero, and `port` must be a non-zero
/// subset of `R/W/X` permission bits. The mapping must not overlap any existing area.
/// Returns `0` on success and `-1` otherwise.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    if mmap_current_task(start, len, port) {
        0
    } else {
        -1
    }
}

/// Remove a user anonymous mapping from the current task's address space.
///
/// `start` and `len` must describe a page-aligned range that is fully covered by mmap'ed
/// pages. Returns `0` on success and `-1` otherwise.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    if munmap_current_task(start, len) {
        0
    } else {
        -1
    }
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
