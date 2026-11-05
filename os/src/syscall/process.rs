//! Process management syscalls
//!
use alloc::sync::Arc;

use crate::config::BIG_STRIDE;
use crate::mm::{translated_byte_buffer, MapPermission, StepByOne, VirtAddr};
use crate::timer::get_time_us;
use crate::{
    fs::{open_file, OpenFlags},
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let ms = get_time_us();
    let new_ms = TimeVal {
        sec: ms / 1_000_000,
        usec: ms % 1_000_000,
    };
    let buffers = translated_byte_buffer(
        current_user_token(),
        _ts as *const u8,
        core::mem::size_of::<TimeVal>(),
    );
    let src = unsafe {
        core::slice::from_raw_parts(
            &new_ms as *const _ as *const u8,
            core::mem::size_of::<TimeVal>(),
        )
    };
    let mut off = 0;
    for buffer in buffers {
        let n = buffer.len();
        buffer.copy_from_slice(&src[off..off + n]);
        off += n;
    }
    0
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    let current_task = current_task().unwrap();
    let mut inner = current_task.inner_exclusive_access();
    let _start_va = VirtAddr::from(_start);
    let _end_va = VirtAddr::from(_start + _len);
    if !_start_va.aligned() {
        return -1;
    }
    let mut vpn = _start_va.floor();
    let end_vpn = _end_va.ceil();
    let mut perm = MapPermission::U;
    if _port & 1 != 0 {
        perm |= MapPermission::R;
    }
    if _port & 2 != 0 {
        perm |= MapPermission::W;
    }
    if _port & 4 != 0 {
        perm |= MapPermission::X;
    }
    if _port & !0x7 != 0 {
        return -1;
    }
    if _port & 0x7 == 0 {
        return -1;
    }
    while vpn < end_vpn {
        let entry = inner.memory_set.translate(vpn);
        match entry {
            Some(entry) => {
                if entry.is_valid() {
                    return -1;
                }
            }
            None => {}
        }
        vpn.step();
    }
    inner
        .memory_set
        .insert_framed_area(_start_va, _end_va, perm);
    0
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    let current_task = current_task().unwrap();
    let mut inner = current_task.inner_exclusive_access();
    let _start_vpn = VirtAddr::from(_start);
    let _end_vpn = VirtAddr::from(_start + _len);
    let start_vpn = _start_vpn.floor();
    if !_start_vpn.aligned() || !_end_vpn.aligned() {
        return -1;
    }
    inner.memory_set.remove_area_with_start_vpn(start_vpn);
    0
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        let new_task = task.spawn(all_data.as_slice());
        add_task(new_task.clone());
        new_task.pid.0 as isize
    } else {
        -1
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    if _prio <= 1 {
        return -1;
    }
    let current_task = current_task().unwrap();
    let mut inner = current_task.inner_exclusive_access();
    inner.prio = _prio;
    inner.pass = BIG_STRIDE / (_prio as usize);
    _prio
}
