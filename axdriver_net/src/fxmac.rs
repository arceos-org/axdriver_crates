use crate::{EthernetAddress, NetBufPtr, NetDriverOps};
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use axdriver_base::{BaseDriverOps, DevError, DevResult, DeviceType};
use core::ptr::NonNull;

pub use fxmac_rs::KernelFunc;
use fxmac_rs::{self, xmac_init, FXmac, FXmacLwipPortTx, FXmacRecvHandler};
use log::*;

extern crate alloc;

const QS: usize = 64;
//const NET_BUF_LEN: usize = 1526;

/// fxmac driver device
pub struct FXmacNic {
    inner: &'static mut FXmac,
    rx_buffer_queue: VecDeque<NetBufPtr>,
}

unsafe impl Sync for FXmacNic {}
unsafe impl Send for FXmacNic {}

impl FXmacNic {
    /// initialize fxmac driver
    pub fn init(mapped_regs: usize) -> DevResult<Self> {
        info!("FXmacNic init @ {:#x}", mapped_regs);
        let rx_buffer_queue = VecDeque::with_capacity(QS);
        let hwaddr: [u8; 6] = [0x98, 0x0e, 0x24, 0x00, 0x11, 0x0];
        let inner = xmac_init(&hwaddr);

        let dev = Self {
            inner,
            rx_buffer_queue,
        };
        Ok(dev)
    }
}

impl BaseDriverOps for FXmacNic {
    fn device_name(&self) -> &str {
        "cdns,phytium-gem-1.0"
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Net
    }
}

impl NetDriverOps for FXmacNic {
    fn mac_address(&self) -> EthernetAddress {
        EthernetAddress([0x98, 0x0e, 0x24, 0x00, 0x11, 0x0])
    }

    fn rx_queue_size(&self) -> usize {
        QS
    }

    fn tx_queue_size(&self) -> usize {
        QS
    }

    fn can_receive(&self) -> bool {
        !self.rx_buffer_queue.is_empty()
    }

    fn can_transmit(&self) -> bool {
        //!self.free_tx_bufs.is_empty()
        true
    }

    fn recycle_rx_buffer(&mut self, rx_buf: NetBufPtr) -> DevResult {
        unsafe {
            drop(Box::from_raw(rx_buf.raw_ptr::<u8>()));
        }
        drop(rx_buf);
        Ok(())
    }

    fn recycle_tx_buffers(&mut self) -> DevResult {
        // drop tx_buf
        Ok(())
    }

    fn receive(&mut self) -> DevResult<NetBufPtr> {
        if !self.rx_buffer_queue.is_empty() {
            // RX buffer have received packets.
            Ok(self.rx_buffer_queue.pop_front().unwrap())
        } else {
            match FXmacRecvHandler(self.inner) {
                None => Err(DevError::Again),
                Some(packets) => {
                    for packet in packets {
                        debug!("received packet length {}", packet.len());
                        let mut buf = Box::new(packet);
                        let buf_ptr = buf.as_mut_ptr() as *mut u8;
                        let buf_len = buf.len();
                        let rx_buf = NetBufPtr::new(
                            NonNull::new(Box::into_raw(buf) as *mut u8).unwrap(),
                            NonNull::new(buf_ptr).unwrap(),
                            buf_len,
                        );

                        self.rx_buffer_queue.push_back(rx_buf);
                    }

                    Ok(self.rx_buffer_queue.pop_front().unwrap())
                }
            }
        }
    }

    fn transmit(&mut self, tx_buf: NetBufPtr) -> DevResult {
        let mut tx_vec = Vec::new();
        tx_vec.push(tx_buf.packet().to_vec());
        let ret = FXmacLwipPortTx(self.inner, tx_vec);
        unsafe {
            drop(Box::from_raw(tx_buf.raw_ptr::<u8>()));
        }
        if ret < 0 {
            Err(DevError::Again)
        } else {
            Ok(())
        }
    }

    fn alloc_tx_buffer(&mut self, size: usize) -> DevResult<NetBufPtr> {
        let mut tx_buf = Box::new(alloc::vec![0; size]);
        let tx_buf_ptr = tx_buf.as_mut_ptr();

        Ok(NetBufPtr::new(
            NonNull::new(Box::into_raw(tx_buf) as *mut u8).unwrap(),
            NonNull::new(tx_buf_ptr).unwrap(),
            size,
        ))
    }
}
