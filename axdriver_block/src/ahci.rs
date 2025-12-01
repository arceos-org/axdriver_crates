use axdriver_base::{BaseDriverOps, DevError, DevResult, DeviceType};
use simple_ahci::{AhciDriver as SimpleAhciDriver, Hal};

use crate::BlockDriverOps;

/// AHCI driver based on the `simple_ahci` crate.
pub struct AhciDriver<H: Hal>(pub SimpleAhciDriver<H>);

/// Safety:
/// - `Send`: The driver takes ownership of the MMIO region and can be safely moved between threads.
/// - `Sync`: The driver's mutating operations require `&mut self`, ensuring exclusive access.
///   Read-only operations (like getting block size) are safe to perform concurrently.
unsafe impl<H: Hal> Send for AhciDriver<H> {}
unsafe impl<H: Hal> Sync for AhciDriver<H> {}

impl<H: Hal> AhciDriver<H> {
    /// Try to construct a new AHCI driver from the given physical/virtual base.
    pub fn try_new(base: usize) -> Option<Self> {
        SimpleAhciDriver::<H>::try_new(base).map(AhciDriver)
    }
}

impl<H: Hal> BaseDriverOps for AhciDriver<H> {
    fn device_name(&self) -> &str {
        "ahci"
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }
}

impl<H: Hal> BlockDriverOps for AhciDriver<H> {
    fn block_size(&self) -> usize {
        self.0.block_size()
    }

    fn num_blocks(&self) -> u64 {
        self.0.capacity()
    }

    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> DevResult {
        if self.0.read(block_id, buf) {
            Ok(())
        } else {
            Err(DevError::Io)
        }
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> DevResult {
        if self.0.write(block_id, buf) {
            Ok(())
        } else {
            Err(DevError::Io)
        }
    }

    fn flush(&mut self) -> DevResult {
        Ok(())
    }
}
