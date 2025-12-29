//! SD/MMC driver based on SDIO.

use axdriver_base::{BaseDriverOps, DevError, DevResult, DeviceType};
use simple_sdmmc::SdMmc;

use crate::BlockDriverOps;

/// A SD/MMC driver.
pub struct SdMmcDriver(SdMmc);

impl SdMmcDriver {
    /// Creates a new [`SdMmcDriver`] from the given base address.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `base` is a valid pointer to the SD/MMC
    /// controller's register block and that no other code is concurrently
    /// accessing the same hardware.
    pub unsafe fn new(base: usize) -> Self {
        Self(SdMmc::new(base))
    }
}

impl BaseDriverOps for SdMmcDriver {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn device_name(&self) -> &str {
        "sdmmc"
    }
}

impl BlockDriverOps for SdMmcDriver {
    fn num_blocks(&self) -> u64 {
        self.0.num_blocks()
    }

    fn block_size(&self) -> usize {
        SdMmc::BLOCK_SIZE
    }

    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> DevResult {
        let (blocks, remainder) = buf.as_chunks_mut::<{ SdMmc::BLOCK_SIZE }>();

        // the buf length must be a multiple of block size
        if !remainder.is_empty() {
            return Err(DevError::InvalidParam);
        }

        // check if block id exceeds device capacity
        if block_id.saturating_add(blocks.len() as u64) > self.0.num_blocks(){
            return Err(DevError::InvalidParam);
        }

        let block_id: u32 = block_id.try_into().map_err(|_| DevError::InvalidParam)?;
        for (i, block) in blocks.iter_mut().enumerate() {
            self.0.read_block(block_id + i as u32, block);
        }

        Ok(())
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> DevResult {
        let (blocks, remainder) = buf.as_chunks::<{ SdMmc::BLOCK_SIZE }>();

        // the buf length must be a multiple of block size
        if !remainder.is_empty() {
            return Err(DevError::InvalidParam);
        }

        // check if block id exceeds device capacity
        if block_id.saturating_add(blocks.len() as u64) > self.0.num_blocks(){
            return Err(DevError::InvalidParam);
        }

        let block_id: u32 = block_id.try_into().map_err(|_| DevError::InvalidParam)?;
        for (i, block) in blocks.iter().enumerate() {
            self.0.write_block(block_id + i as u32, block);
        }

        Ok(())
    }

    fn flush(&mut self) -> DevResult {
        Ok(())
    }
}
