use crate::error::{Error, ErrorKind};

/// The call of slang-rhi whose error [`from_rhi`] maps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Call {
    /// The creation of the device of slang-rhi.
    NewDevice,
    /// Any other call.
    Other,
}

/// Returns the error of `operation` for an error of slang-rhi from `call`,
/// with the error of slang-rhi as its source.
///
/// A failure to create the device is [`ErrorKind::NoAdapter`] when slang-rhi
/// finds no device and [`ErrorKind::DeviceRequest`] otherwise. A failure of
/// Slang, or to compile for the device, is [`ErrorKind::Compile`], what
/// slang-rhi or the device does not support is [`ErrorKind::Unsupported`],
/// and every other failure is [`ErrorKind::Backend`], because the crate
/// checks every argument before it calls slang-rhi.
pub(super) fn from_rhi(operation: &'static str, call: Call, error: slang_rhi::Error) -> Error {
    let kind = match (call, error.kind()) {
        (Call::NewDevice, slang_rhi::ErrorKind::NoDevice) => ErrorKind::NoAdapter,
        (Call::NewDevice, _) => ErrorKind::DeviceRequest,
        (Call::Other, slang_rhi::ErrorKind::Compile | slang_rhi::ErrorKind::Slang) => {
            ErrorKind::Compile
        }
        (Call::Other, slang_rhi::ErrorKind::Unsupported) => ErrorKind::Unsupported,
        (Call::Other, _) => ErrorKind::Backend,
    };
    Error::new(kind, operation).with_source(error)
}
