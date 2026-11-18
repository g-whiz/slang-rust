use std::borrow::Cow;
use std::ffi::{CStr, c_char};
use std::fmt;
use std::sync::Arc;

use slang_rhi_sys::{IDebugCallback, IDebugCallbackVtbl};

use crate::guard;

/// The closure that receives the messages of a device.
pub(crate) type Callback = dyn Fn(&DebugMessage<'_>) + Send + Sync;

/// The kind of a message of a device.
///
/// The enumeration is non-exhaustive, so a later version can add kinds, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`DeviceOptions::debug_callback`](crate::DeviceOptions::debug_callback).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DebugMessageType {
    /// Information.
    Info,
    /// A warning.
    Warning,
    /// An error.
    Error,
}

impl DebugMessageType {
    fn from_raw(raw: slang_rhi_sys::DebugMessageType) -> Option<Self> {
        match raw {
            slang_rhi_sys::DebugMessageType::Info => Some(Self::Info),
            slang_rhi_sys::DebugMessageType::Warning => Some(Self::Warning),
            slang_rhi_sys::DebugMessageType::Error => Some(Self::Error),
            _ => None,
        }
    }
}

/// The part of the system that reported a message of a device.
///
/// The enumeration is non-exhaustive, so a later version can add sources, and
/// a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`DeviceOptions::debug_callback`](crate::DeviceOptions::debug_callback).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DebugMessageSource {
    /// The validation layer of the graphics API, or slang-rhi's own
    /// validation.
    Layer,
    /// The graphics driver.
    Driver,
    /// Slang, the shader compiler, such as an error in a shader that
    /// slang-rhi compiles.
    Slang,
}

impl DebugMessageSource {
    fn from_raw(raw: slang_rhi_sys::DebugMessageSource) -> Option<Self> {
        match raw {
            slang_rhi_sys::DebugMessageSource::Layer => Some(Self::Layer),
            slang_rhi_sys::DebugMessageSource::Driver => Some(Self::Driver),
            slang_rhi_sys::DebugMessageSource::Slang => Some(Self::Slang),
            _ => None,
        }
    }
}

/// One message that a device reported to its debug callback.
///
/// The message borrows text that slang-rhi keeps valid only during the call
/// of the callback, so the callback copies what it keeps.
///
/// # Examples
///
/// See the example of [`DeviceOptions::debug_callback`](crate::DeviceOptions::debug_callback).
pub struct DebugMessage<'a> {
    message_type: DebugMessageType,
    source: DebugMessageSource,
    text: &'a CStr,
}

impl DebugMessage<'_> {
    /// Returns the kind of the message.
    ///
    /// # Examples
    ///
    /// See the example of [`DeviceOptions::debug_callback`](crate::DeviceOptions::debug_callback).
    #[must_use]
    pub fn message_type(&self) -> DebugMessageType {
        self.message_type
    }

    /// Returns the part of the system that reported the message.
    ///
    /// # Examples
    ///
    /// See the example of [`DeviceOptions::debug_callback`](crate::DeviceOptions::debug_callback).
    #[must_use]
    pub fn source(&self) -> DebugMessageSource {
        self.source
    }

    /// Returns the text of the message, with each sequence that is not valid
    /// UTF-8 replaced by U+FFFD, the replacement character.
    ///
    /// # Examples
    ///
    /// See the example of [`DeviceOptions::debug_callback`](crate::DeviceOptions::debug_callback).
    #[must_use]
    pub fn text(&self) -> Cow<'_, str> {
        self.text.to_string_lossy()
    }
}

impl fmt::Debug for DebugMessage<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DebugMessage")
            .field("message_type", &self.message_type)
            .field("source", &self.source)
            .field("text", &self.text())
            .finish()
    }
}

/// The debug callback object of one device, which slang-rhi calls through
/// its first field.
///
/// The crate never frees it, because slang-rhi gives no notice when the
/// native device that names it is freed.
#[repr(C)]
struct CallbackObject {
    base: IDebugCallback,
    user: Option<Arc<Callback>>,
}

const fn unwinding(
    function: unsafe extern "system" fn(
        *mut IDebugCallback,
        slang_rhi_sys::DebugMessageType,
        slang_rhi_sys::DebugMessageSource,
        *const c_char,
    ),
) -> unsafe extern "system-unwind" fn(
    *mut IDebugCallback,
    slang_rhi_sys::DebugMessageType,
    slang_rhi_sys::DebugMessageSource,
    *const c_char,
) {
    // SAFETY: the two types differ only in the unwinding form of one calling
    // convention, and a caller through the unwinding form may call a
    // function of the non-unwinding form, as the standard library documents
    // for function pointers.
    unsafe {
        std::mem::transmute::<
            unsafe extern "system" fn(
                *mut IDebugCallback,
                slang_rhi_sys::DebugMessageType,
                slang_rhi_sys::DebugMessageSource,
                *const c_char,
            ),
            unsafe extern "system-unwind" fn(
                *mut IDebugCallback,
                slang_rhi_sys::DebugMessageType,
                slang_rhi_sys::DebugMessageSource,
                *const c_char,
            ),
        >(function)
    }
}

static VTABLE: IDebugCallbackVtbl = IDebugCallbackVtbl {
    handleMessage: unwinding(handle_message),
};

/// Receives one message of a device.
///
/// It keeps a message of Slang for the call of this crate in progress on the
/// thread, and passes every message whose kind and source the crate knows to
/// the user's closure. A panic of the closure reaches the non-unwinding
/// boundary of this function and aborts the process.
unsafe extern "system" fn handle_message(
    this: *mut IDebugCallback,
    message_type: slang_rhi_sys::DebugMessageType,
    source: slang_rhi_sys::DebugMessageSource,
    text: *const c_char,
) {
    if this.is_null() || text.is_null() {
        return;
    }
    // SAFETY: slang-rhi calls this function only through the vtable of a
    // `CallbackObject` that `leak` created and never frees, with the pointer
    // that `leak` returned, which covers the whole object. The object is only
    // read, so references from several threads at once are sound.
    let object = unsafe { &*this.cast::<CallbackObject>() };
    // SAFETY: slang-rhi passes a NUL-terminated message that stays valid for
    // the duration of the call, and the reference does not outlive the call.
    let text = unsafe { CStr::from_ptr(text) };
    if source == slang_rhi_sys::DebugMessageSource::Slang {
        guard::collect(&text.to_string_lossy());
    }
    let (Some(message_type), Some(source)) = (
        DebugMessageType::from_raw(message_type),
        DebugMessageSource::from_raw(source),
    ) else {
        return;
    };
    if let Some(user) = &object.user {
        user(&DebugMessage {
            message_type,
            source,
            text,
        });
    }
}

/// Creates the debug callback object of one device, which the crate never
/// frees, and returns the pointer that the device receives.
pub(crate) fn leak(user: Option<Arc<Callback>>) -> *mut IDebugCallback {
    let object = Box::new(CallbackObject {
        base: IDebugCallback {
            vtable: &raw const VTABLE,
        },
        user,
    });
    Box::into_raw(object).cast::<IDebugCallback>()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, PoisonError};

    use slang_rhi_sys::IDebugCallback;

    use super::{
        Callback, CallbackObject, DebugMessage, DebugMessageSource, DebugMessageType, leak,
    };
    use crate::guard;

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn the_kinds_and_sources_of_messages_are_send_and_sync() {
        assert_send_sync::<DebugMessageType>();
        assert_send_sync::<DebugMessageSource>();
    }

    /// Calls the callback object through its vtable, as slang-rhi does.
    fn deliver(
        object: *mut IDebugCallback,
        message_type: slang_rhi_sys::DebugMessageType,
        source: slang_rhi_sys::DebugMessageSource,
        text: &std::ffi::CStr,
    ) {
        // SAFETY: `object` comes from `leak` and is freed only at the end of
        // the test, and the message is a live NUL-terminated string.
        unsafe { IDebugCallback::handleMessage(object, message_type, source, text.as_ptr()) };
    }

    /// Frees a callback object that `leak` created.
    ///
    /// # Safety
    ///
    /// `object` comes from `leak`, no device names it, and it is freed once.
    /// Breaking any of them is undefined behaviour.
    unsafe fn free(object: *mut IDebugCallback) {
        // SAFETY: the caller guarantees that `leak` allocated the object as a
        // `Box<CallbackObject>` and that nothing uses it afterwards.
        drop(unsafe { Box::from_raw(object.cast::<CallbackObject>()) });
    }

    #[test]
    fn a_message_reaches_the_closure_with_its_kind_source_and_text() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&seen);
        let user: Arc<Callback> = Arc::new(move |message: &DebugMessage<'_>| {
            record.lock().unwrap_or_else(PoisonError::into_inner).push((
                message.message_type(),
                message.source(),
                message.text().into_owned(),
            ));
        });
        let object = leak(Some(user));
        deliver(
            object,
            slang_rhi_sys::DebugMessageType::Warning,
            slang_rhi_sys::DebugMessageSource::Driver,
            c"slow path",
        );
        deliver(
            object,
            slang_rhi_sys::DebugMessageType(9),
            slang_rhi_sys::DebugMessageSource::Driver,
            c"unknown kind",
        );
        deliver(
            object,
            slang_rhi_sys::DebugMessageType::Error,
            slang_rhi_sys::DebugMessageSource::Slang,
            c"bad \xff byte",
        );
        // SAFETY: no device names the object, and the test frees it once.
        unsafe { free(object) };
        assert_eq!(
            *seen.lock().unwrap_or_else(PoisonError::into_inner),
            [
                (
                    DebugMessageType::Warning,
                    DebugMessageSource::Driver,
                    String::from("slow path")
                ),
                (
                    DebugMessageType::Error,
                    DebugMessageSource::Slang,
                    String::from("bad \u{fffd} byte")
                ),
            ]
        );
    }

    #[test]
    fn a_message_of_slang_is_kept_for_the_call_in_progress_without_a_closure() {
        let object = leak(None);
        let calls = AtomicUsize::new(0);
        let ((), messages) = guard::call_collecting("Test::compile", || {
            calls.fetch_add(1, Ordering::Relaxed);
            deliver(
                object,
                slang_rhi_sys::DebugMessageType::Error,
                slang_rhi_sys::DebugMessageSource::Slang,
                c"error[E30015]: undefined identifier",
            );
            deliver(
                object,
                slang_rhi_sys::DebugMessageType::Info,
                slang_rhi_sys::DebugMessageSource::Layer,
                c"not from Slang",
            );
        });
        // SAFETY: no device names the object, and the test frees it once.
        unsafe { free(object) };
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(messages, ["error[E30015]: undefined identifier"]);
    }
}
