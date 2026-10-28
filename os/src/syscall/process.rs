//! Process management syscalls
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::get_time_us;
use crate::task::current_user_token;
use crate::mm::translated_byte_buffer;
use crate::mm::{PageTable, VirtAddr};
use crate::task::get_current_syscall_times;
use crate::mm::MapPermission;
use crate::task::{mmap_current,munmap_current};

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

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let us = get_time_us();
    let token = current_user_token();
    let timeval = TimeVal{
        sec: us/1_000_000,
        usec: us % 1_000_000,
    };
    let src = unsafe{
        core::slice::from_raw_parts(&timeval as *const TimeVal as *const u8,core::mem::size_of::<TimeVal>())
    };
    let buffers = translated_byte_buffer(token,_ts as *const u8,core::mem::size_of::<TimeVal>());
    let mut offset = 0;
    for buffer in buffers{
        let len = buffer.len();
        buffer.copy_from_slice(&src[offset..offset+len]);
        offset += len;
    }
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    match _trace_request{
        0 => {
            // 1. id 是用户虚拟地址
            // 2. 找当前进程页表
            // 3. 查 id 所在页
            // 4. 检查 U + R
            // 5. 找到物理地址
            // 6. 读取一个 u8
            let page_table = PageTable::from_token(current_user_token());
            let va = VirtAddr::from(_id);
            // 检查 _id 是否本来就是合法的 Sv39 地址
            let canonical: usize = va.into();
            if canonical != _id {
                return -1;
            }
            //找到 id 属于哪一个虚拟页
            let vpn = va.floor();
            //根据vpn来从页表中取得页表项
            let pte = match page_table.translate(vpn){
                Some(pte) => pte,
                None => return -1,
            };
            
            //到了这里我们已经得到了页表项，我们接下来处理这个页表项
            //flag就是标志位，ppn就是物理页号
            if !pte.is_valid() || !pte.usercansee() || !pte.readable(){
                return -1;
            }
            let ppn = pte.ppn();
            //我们有了页的地址，有了偏移量，没必要多此一举拼成完整的地址
            let bytes = ppn.get_bytes_array();
            let value = bytes[va.page_offset()];
            value as isize
        }
        1 => {
            let page_table = PageTable::from_token(current_user_token());
            let va = VirtAddr::from(_id);
            // 检查 _id 是否本来就是合法的 Sv39 地址
            let canonical: usize = va.into();
            if canonical != _id {
                return -1;
            }
            //找到 id 属于哪一个虚拟页
            let vpn = va.floor();
            //根据vpn来从页表中取得页表项
            let pte = match page_table.translate(vpn){
                Some(pte) => pte,
                None => return -1,
            };
            //到了这里我们已经得到了页表项，我们接下来处理这个页表项
            //flag就是标志位，ppn就是物理页号
            if !pte.is_valid() || !pte.usercansee() || !pte.writable(){
                return -1;
            }
            let ppn = pte.ppn();
            //我们有了页的地址，有了偏移量，没必要多此一举拼成完整的地址
            let bytes = ppn.get_bytes_array();
            bytes[va.page_offset()] = _data as u8;
            0
        }
        2 => {
            get_current_syscall_times(_id) as isize
        }
        _ => -1
    }
}

// YOUR JOB: Implement mmap.
///
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    let start_va = VirtAddr::from(_start);
    if !start_va.aligned(){
        return -1;
    }
    let end_va = VirtAddr::from(_start+_len);
    if !(1..=7).contains(&_port){
        return -1;
    }
    let permission = MapPermission::from_bits((_port << 1) as u8).unwrap() | MapPermission::U;
    if !mmap_current(start_va,end_va,permission){
        return -1;
    }

    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    let start_va = VirtAddr::from(_start);
    if !start_va.aligned(){
        return -1;
    }
    let end_va = VirtAddr::from(_start+_len);
    if !munmap_current(start_va,end_va){
        return -1;
    }
    0
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
