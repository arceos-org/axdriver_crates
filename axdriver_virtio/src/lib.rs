//! Wrappers of some devices in the [`virtio-drivers`][1] crate, that implement
//! traits in the [`axdriver_base`][2] series crates.
//!
//! Like the [`virtio-drivers`][1] crate, you must implement the [`VirtIoHal`]
//! trait (alias of [`virtio-drivers::Hal`][3]), to allocate DMA regions and
//! translate between physical addresses (as seen by devices) and virtual
//! addresses (as seen by your program).
//!
//! [1]: https://docs.rs/virtio-drivers/latest/virtio_drivers/
//! [2]: https://github.com/arceos-org/axdriver_crates/tree/main/axdriver_base
//! [3]: https://docs.rs/virtio-drivers/latest/virtio_drivers/trait.Hal.html

#![no_std]
#![cfg_attr(doc, feature(doc_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "block")]
mod blk;
#[cfg(feature = "block")]
pub use self::blk::VirtIoBlkDev;

#[cfg(feature = "gpu")]
mod gpu;
#[cfg(feature = "gpu")]
pub use self::gpu::VirtIoGpuDev;

#[cfg(feature = "input")]
mod input;
#[cfg(feature = "input")]
pub use self::input::VirtIoInputDev;

#[cfg(feature = "net")]
mod net;
#[cfg(feature = "net")]
pub use self::net::VirtIoNetDev;

#[cfg(feature = "socket")]
mod socket;
#[cfg(feature = "socket")]
pub use self::socket::VirtIoSocketDev;

pub use virtio_drivers::transport::pci::bus as pci;
pub use virtio_drivers::transport::{mmio::MmioTransport, pci::PciTransport, Transport};
pub use virtio_drivers::{BufferDirection, Hal as VirtIoHal, PhysAddr};

use self::pci::{ConfigurationAccess, DeviceFunction, DeviceFunctionInfo, PciRoot};
use axdriver_base::{DevError, DeviceType};
use virtio_drivers::transport::DeviceType as VirtIoDevType;

/// Try to probe a VirtIO MMIO device from the given memory region.
///
/// If the device is recognized, returns the device type and a transport object
/// for later operations. Otherwise, returns [`None`].
pub fn probe_mmio_device(
    reg_base: *mut u8,
    reg_size: usize,
) -> Option<(DeviceType, MmioTransport<'static>)> {
    use core::ptr::NonNull;
    use virtio_drivers::transport::mmio::VirtIOHeader;

    let header = NonNull::new(reg_base as *mut VirtIOHeader)?;
    let transport = unsafe { MmioTransport::new(header, reg_size) }.ok()?;
    let dev_type = as_dev_type(transport.device_type())?;
    Some((dev_type, transport))
}

/// Try to probe a VirtIO PCI device from the given PCI address.
///
/// If the device is recognized, returns the device type and a transport object
/// for later operations. Otherwise, returns [`None`].
pub fn probe_pci_device<H: VirtIoHal, C: ConfigurationAccess>(
    root: &mut PciRoot<C>,
    bdf: DeviceFunction,
    dev_info: &DeviceFunctionInfo,
) -> Option<(DeviceType, PciTransport, usize)> {
    use virtio_drivers::transport::pci::virtio_device_type;

    let dev_type = virtio_device_type(dev_info).and_then(as_dev_type)?;
    #[cfg(target_arch = "x86_64")]
    let irq = legacy_irq_for_bdf(&root.configuration_access, bdf);

    #[cfg(not(target_arch = "x86_64"))]
    let irq = {
        #[cfg(target_arch = "loongarch64")]
        const PCI_IRQ_BASE: usize = 0x10;
        #[cfg(target_arch = "aarch64")]
        const PCI_IRQ_BASE: usize = 0x23;
        #[cfg(target_arch = "riscv64")]
        const PCI_IRQ_BASE: usize = 0x20;
        PCI_IRQ_BASE + (bdf.device & 3) as usize
    };

    let transport = PciTransport::new::<H, C>(root, bdf).ok()?;

    #[cfg(target_arch = "x86_64")]
    if irq == 0 || irq == 0xff {
        log::warn!(
            "PCI device {:?}: Interrupt Line not assigned ({:#x})",
            bdf,
            irq
        );
        return None;
    }

    Some((dev_type, transport, irq))
}

/// Reads the PCI Interrupt Line register (config space offset 0x3C) for the
/// given device and returns it as a legacy IRQ number.
///
/// Returns 0xFF if the register has not been programmed by firmware, which
/// means the device has no usable legacy IRQ assignment. The caller should
/// treat 0xFF as "no IRQ".
#[cfg(target_arch = "x86_64")]
#[inline]
fn legacy_irq_for_bdf<C: ConfigurationAccess>(config: &C, bdf: DeviceFunction) -> usize {
    let word = config.read_word(bdf, 0x3C);
    (word & 0xFF) as usize
}

const fn as_dev_type(t: VirtIoDevType) -> Option<DeviceType> {
    use VirtIoDevType::*;
    match t {
        Block => Some(DeviceType::Block),
        Network => Some(DeviceType::Net),
        GPU => Some(DeviceType::Display),
        Input => Some(DeviceType::Input),
        Socket => Some(DeviceType::Vsock),
        _ => None,
    }
}

#[allow(dead_code)]
const fn as_dev_err(e: virtio_drivers::Error) -> DevError {
    use virtio_drivers::device::socket::SocketError::*;
    use virtio_drivers::Error::*;
    match e {
        QueueFull => DevError::BadState,
        NotReady => DevError::Again,
        WrongToken => DevError::BadState,
        AlreadyUsed => DevError::AlreadyExists,
        InvalidParam => DevError::InvalidParam,
        DmaError => DevError::NoMemory,
        IoError => DevError::Io,
        Unsupported => DevError::Unsupported,
        ConfigSpaceTooSmall => DevError::BadState,
        ConfigSpaceMissing => DevError::BadState,
        SocketDeviceError(e) => match e {
            ConnectionExists => DevError::AlreadyExists,
            NotConnected => DevError::BadState,
            InvalidOperation | InvalidNumber | UnknownOperation(_) => DevError::InvalidParam,
            OutputBufferTooShort(_) | BufferTooShort | BufferTooLong(_, _) => {
                DevError::InvalidParam
            }
            UnexpectedDataInPacket | PeerSocketShutdown => DevError::Io,
            InsufficientBufferSpaceInPeer => DevError::Again,
            RecycledWrongBuffer => DevError::BadState,
        },
    }
}
