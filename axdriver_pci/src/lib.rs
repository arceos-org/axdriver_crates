//! Structures and functions for PCI bus operations.
//!
//! Currently, it just re-exports structures from the crate [virtio-drivers][1]
//! and its module [`virtio_drivers::transport::pci::bus`][2].
//!
//! [1]: https://docs.rs/virtio-drivers/latest/virtio_drivers/
//! [2]: https://docs.rs/virtio-drivers/latest/virtio_drivers/transport/pci/bus/index.html

#![no_std]

pub use virtio_drivers::transport::pci::bus::{
    BarInfo, Cam, CapabilityInfo, Command, ConfigurationAccess, DeviceFunction, DeviceFunctionInfo,
    HeaderType, MemoryBarType, MmioCam, PciError, PciRoot, Status,
};

/// Provides read/write access to PCI configuration space registers.
///
/// The `virtio-drivers` crate exposes `PciRoot` but keeps its internal
/// config-space access helpers private. This type re-implements the same MMIO
/// access so callers can read arbitrary PCI config registers such as
/// `Interrupt Line`.
pub struct PciConfigAccess {
    mmio_base: *mut u32,
    cam: Cam,
}

unsafe impl Send for PciConfigAccess {}
unsafe impl Sync for PciConfigAccess {}

impl PciConfigAccess {
    /// Creates a new PCI config-space accessor from the ECAM/MMIO base address.
    ///
    /// # Safety
    ///
    /// `mmio_base` must point to a valid mapped PCI configuration window.
    pub unsafe fn new(mmio_base: *mut u8, cam: Cam) -> Self {
        assert!(mmio_base as usize & 0x3 == 0);
        Self {
            mmio_base: mmio_base as *mut u32,
            cam,
        }
    }

    fn cam_offset(&self, device_function: DeviceFunction, register_offset: u8) -> u32 {
        assert!(device_function.valid());

        let bdf = (device_function.bus as u32) << 8
            | (device_function.device as u32) << 3
            | device_function.function as u32;
        let address =
            bdf << match self.cam {
                Cam::MmioCam => 8,
                Cam::Ecam => 12,
            } | (register_offset as u32 & !0x3);
        assert!(address < self.cam.size());
        assert!(address & 0x3 == 0);
        address
    }

    /// Reads a 32-bit word from PCI configuration space.
    pub fn read_word(&self, device_function: DeviceFunction, register_offset: u8) -> u32 {
        let address = self.cam_offset(device_function, register_offset);
        unsafe { self.mmio_base.add((address >> 2) as usize).read_volatile() }
    }

    /// Writes a 32-bit word to PCI configuration space.
    pub fn write_word(&mut self, device_function: DeviceFunction, register_offset: u8, data: u32) {
        let address = self.cam_offset(device_function, register_offset);
        unsafe { self.mmio_base.add((address >> 2) as usize).write_volatile(data) }
    }
}

/// Used to allocate MMIO regions for PCI BARs.
pub struct PciRangeAllocator {
    _start: u64,
    end: u64,
    current: u64,
}

impl PciRangeAllocator {
    /// Creates a new allocator from a memory range.
    pub const fn new(base: u64, size: u64) -> Self {
        Self {
            _start: base,
            end: base + size,
            current: base,
        }
    }

    /// Allocates a memory region with the given size.
    ///
    /// The `size` should be a power of 2, and the returned value is also a
    /// multiple of `size`.
    pub fn alloc(&mut self, size: u64) -> Option<u64> {
        if !size.is_power_of_two() {
            return None;
        }
        let ret = align_up(self.current, size);
        if ret + size > self.end {
            return None;
        }

        self.current = ret + size;
        Some(ret)
    }
}

const fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}
