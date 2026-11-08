use crate::SlangReflectionVariable;
use crate::spReflectionVariable_GetName;

use crate::reflection::ty::Type;
use crate::root::spReflectionVariable_GetType;
use std::ffi::CStr;

#[repr(transparent)]
pub struct Variable(SlangReflectionVariable);

impl Variable {
    unsafe fn as_mut_slang_ptr(&self) -> *mut SlangReflectionVariable {
        &self.0 as *const _ as *mut _
    }

    /**
    Returns the name of the reflected variable, or None if
     - &self is a null pointer, or if
     - the name contains invalid UTF-8 characters.
    */
    pub fn name(&self) -> Option<&str> {
        unsafe {
            let name_ptr = spReflectionVariable_GetName(self.as_mut_slang_ptr());
            name_ptr
                .as_ref()
                .map(|s_ptr| CStr::from_ptr(s_ptr).to_str().ok())
                .flatten()
        }
    }

    /**
    Returns the Type of the reflected variable, of None if &self is a null pointer.
    */
    pub fn ty(&self) -> Option<&Type> {
        unsafe {
            let type_ptr = spReflectionVariable_GetType(self.as_mut_slang_ptr()) as *mut Type;
            type_ptr.as_ref()
        }
    }
}
