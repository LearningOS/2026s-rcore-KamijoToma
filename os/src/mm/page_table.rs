//! Implementation of [`PageTableEntry`] and [`PageTable`].

use super::{frame_alloc, FrameTracker, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use crate::config::PAGE_SIZE;
use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;
use core::mem::MaybeUninit;

bitflags! {
    /// page table entry flags
    pub struct PTEFlags: u8 {
        /// Valid
        const V = 1 << 0;
        /// Readable
        const R = 1 << 1;
        /// Writable
        const W = 1 << 2;
        /// eXecutable
        const X = 1 << 3;
        /// User
        const U = 1 << 4;
        /// Global
        const G = 1 << 5;
        /// Accessed
        const A = 1 << 6;
        /// Dirty
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
/// page table entry structure
pub struct PageTableEntry {
    /// bits of page table entry
    pub bits: usize,
}

impl PageTableEntry {
    /// Create a new page table entry
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }
    /// Create an empty page table entry
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }
    /// Get the physical page number from the page table entry
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }
    /// Get the flags from the page table entry
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    /// The page pointered by page table entry is valid?
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    /// The page pointered by page table entry is readable?
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    /// The page pointered by page table entry is writable?
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    /// The page pointered by page table entry is executable?
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

/// page table structure
pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

/// Assume that it won't oom when creating/mapping.
impl PageTable {
    /// Create a new page table
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
    /// Temporarily used to get arguments from user space.
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }
    /// Find PageTableEntry by VirtPageNum, create a frame for a 4KB page table if not exist
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }
    /// Find PageTableEntry by VirtPageNum
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }
    /// set the map between virtual page number and physical page number
    #[allow(unused)]
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    /// remove the map between virtual page number and physical page number
    #[allow(unused)]
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }
    /// get the page table entry from the virtual page number
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }
    /// get the token from the page table
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

/// Translate&Copy a ptr[u8] array with LENGTH len to a mutable u8 Vec through page table
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}

fn checked_user_byte_buffers(
    token: usize,
    ptr: *const u8,
    len: usize,
    required_flags: PTEFlags,
) -> Option<Vec<&'static mut [u8]>> {
    if len == 0 {
        return Some(Vec::new());
    }
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start.checked_add(len)?;
    if start != VirtAddr::from(start).0 || end != VirtAddr::from(end).0 {
        return None;
    }
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let pte = page_table.translate(vpn)?;
        if !pte.is_valid() || !pte.flags().contains(required_flags) {
            return None;
        }
        let ppn = pte.ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        let slice_end = if end_va.page_offset() == 0 {
            PAGE_SIZE
        } else {
            end_va.page_offset()
        };
        v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..slice_end]);
        start = end_va.into();
    }
    Some(v)
}

/// Copy bytes from user space into a kernel buffer after validating `R|U` permissions.
///
/// Returns `false` if any page in the range is unmapped, not user-accessible, or lacks read
/// permission.
pub fn copy_from_user(token: usize, ptr: *const u8, dst: &mut [u8]) -> bool {
    let byte_buffers =
        match checked_user_byte_buffers(token, ptr, dst.len(), PTEFlags::R | PTEFlags::U) {
            Some(byte_buffers) => byte_buffers,
            None => return false,
        };
    let mut copied = 0;
    for buffer in byte_buffers {
        let len = buffer.len();
        dst[copied..copied + len].copy_from_slice(buffer);
        copied += len;
    }
    true
}

/// Copy bytes from the kernel into user space after validating `W|U` permissions.
///
/// Returns `false` if any page in the destination range is unmapped, not user-accessible,
/// or lacks write permission.
pub fn copy_to_user(token: usize, ptr: *mut u8, data: &[u8]) -> bool {
    let byte_buffers = match checked_user_byte_buffers(
        token,
        ptr as *const u8,
        data.len(),
        PTEFlags::W | PTEFlags::U,
    ) {
        Some(byte_buffers) => byte_buffers,
        None => return false,
    };
    let mut copied = 0;
    for buffer in byte_buffers {
        let len = buffer.len();
        buffer.copy_from_slice(&data[copied..copied + len]);
        copied += len;
    }
    true
}

/// Read a single byte from user space.
///
/// Returns `None` if the address is not a readable user address.
pub fn read_user_byte(token: usize, ptr: *const u8) -> Option<u8> {
    let mut byte = [0u8; 1];
    if copy_from_user(token, ptr, &mut byte) {
        Some(byte[0])
    } else {
        None
    }
}

/// Write a single byte into user space.
///
/// Returns `false` if the address is not a writable user address.
pub fn write_user_byte(token: usize, ptr: *mut u8, data: u8) -> bool {
    copy_to_user(token, ptr, core::slice::from_ref(&data))
}


/// Translate&Copy a immutable ptr[u8] array with LENGTH len to a immutable u8 Vec through page table to user space
/// This function is used to save the data in the kernel to user space.
#[allow(unused)]
pub fn save_byte_to_user(token: usize, ptr: *mut u8, data: &[u8]) {
    assert!(copy_to_user(token, ptr, data));
}

/// Load a sized data from user space to kernel space through page table, and return it.
/// 
/// Note: This function introduce additional copy
#[allow(unused)]
pub fn load_data_from_user<T: Sized>(token: usize, ptr: *const T) -> T {
    let mut data = MaybeUninit::<T>::uninit();
    let data_bytes = unsafe {
        core::slice::from_raw_parts_mut(
            data.as_mut_ptr() as *mut u8,
            core::mem::size_of::<T>(),
        )
    };
    assert!(copy_from_user(token, ptr as *const u8, data_bytes));
    unsafe { data.assume_init() }
}

/// Save a sized value from kernel space to user space and report whether it succeeds.
///
/// This is the typed counterpart of [`copy_to_user`].
pub fn copy_data_to_user<T: Sized>(token: usize, ptr: *mut T, data: &T) -> bool {
    let byte_buffer = unsafe {
        core::slice::from_raw_parts(
            data as *const T as *const u8,
            core::mem::size_of::<T>(),
        )
    };
    copy_to_user(token, ptr as *mut u8, byte_buffer)
}

/// Save a sized data from kernel space to user space through page table.
#[allow(unused)]
pub fn save_data_to_user<T: Sized>(token: usize, ptr: *mut T, data: &T) {
    assert!(copy_data_to_user(token, ptr, data));
}
