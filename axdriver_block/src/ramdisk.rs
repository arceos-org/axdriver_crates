//! A RAM disk driver backed by heap memory or static slice.

extern crate alloc;

use alloc::alloc::{alloc_zeroed, dealloc};
use core::{
    alloc::Layout,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use axdriver_base::{BaseDriverOps, DevError, DevResult, DeviceType};

use crate::BlockDriverOps;

const BLOCK_SIZE: usize = 512;

/// A RAM disk driver.
pub enum RamDisk {
    /// A RAM disk backed by heap memory.
    Heap(NonNull<[u8]>),
    /// A RAM disk backed by a static slice.
    Static(&'static mut [u8]),
}

unsafe impl Send for RamDisk {}
unsafe impl Sync for RamDisk {}

impl Default for RamDisk {
    fn default() -> Self {
        Self::Heap(NonNull::<[u8; 0]>::dangling())
    }
}

impl RamDisk {
    /// Creates a new RAM disk with the given size hint.
    ///
    /// The actual size of the RAM disk will be aligned upwards to the block
    /// size (512 bytes).
    pub fn new_heap(size_hint: usize) -> Self {
        let size = align_up(size_hint);
        let ptr = unsafe {
            NonNull::new_unchecked(alloc_zeroed(Layout::from_size_align_unchecked(
                size, BLOCK_SIZE,
            )))
        };
        Self::Heap(NonNull::slice_from_raw_parts(ptr, size))
    }

    /// Creates a new RAM disk from the given static buffer.
    ///
    /// # Panics
    /// Panics if the buffer is not aligned to block size or its size is not
    /// a multiple of block size.
    pub fn new_static(buf: &'static mut [u8]) -> Self {
        assert_eq!(buf.as_ptr().addr() & (BLOCK_SIZE - 1), 0);
        assert_eq!(buf.len() % BLOCK_SIZE, 0);
        Self::Static(buf)
    }
}

impl Drop for RamDisk {
    fn drop(&mut self) {
        if let RamDisk::Heap(ptr) = self {
            if ptr.is_empty() {
                return;
            }
            unsafe {
                dealloc(
                    ptr.cast::<u8>().as_ptr(),
                    Layout::from_size_align_unchecked(ptr.len(), BLOCK_SIZE),
                )
            }
        }
    }
}

impl Deref for RamDisk {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            RamDisk::Heap(ptr) => unsafe { ptr.as_ref() },
            RamDisk::Static(slice) => slice,
        }
    }
}

impl DerefMut for RamDisk {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            RamDisk::Heap(ptr) => unsafe { ptr.as_mut() },
            RamDisk::Static(slice) => slice,
        }
    }
}

impl From<&[u8]> for RamDisk {
    fn from(data: &[u8]) -> Self {
        let mut this = RamDisk::new_heap(data.len());
        this[..data.len()].copy_from_slice(data);
        this
    }
}

impl BaseDriverOps for RamDisk {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn device_name(&self) -> &str {
        "ramdisk"
    }
}

impl BlockDriverOps for RamDisk {
    #[inline]
    fn num_blocks(&self) -> u64 {
        (self.len() / BLOCK_SIZE) as u64
    }

    #[inline]
    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> DevResult {
        if buf.len() % BLOCK_SIZE != 0 {
            return Err(DevError::InvalidParam);
        }
        let block_id: usize = block_id.try_into().map_err(|_| DevError::InvalidParam)?;
        let offset = block_id
            .checked_mul(BLOCK_SIZE)
            .ok_or(DevError::InvalidParam)?;
        if offset.saturating_add(buf.len()) > self.len() {
            return Err(DevError::InvalidParam);
        }
        buf.copy_from_slice(&self[offset..offset + buf.len()]);
        Ok(())
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> DevResult {
        if buf.len() % BLOCK_SIZE != 0 {
            return Err(DevError::InvalidParam);
        }
        let block_id: usize = block_id.try_into().map_err(|_| DevError::InvalidParam)?;
        let offset = block_id
            .checked_mul(BLOCK_SIZE)
            .ok_or(DevError::InvalidParam)?;
        if offset.saturating_add(buf.len()) > self.len() {
            return Err(DevError::InvalidParam);
        }
        self[offset..offset + buf.len()].copy_from_slice(buf);
        Ok(())
    }

    fn flush(&mut self) -> DevResult {
        Ok(())
    }
}

const fn align_up(val: usize) -> usize {
    (val + BLOCK_SIZE - 1) & !(BLOCK_SIZE - 1)
}
