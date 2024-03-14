#![allow(dead_code)]

use std::ffi::{CStr, CString};

use crate::bindings::{gboolean, gchar, gpointer};
use crate::common::error::*;

pub const TRUE: gboolean = 1_i32;
pub const FALSE: gboolean = 0_i32;

pub mod util_macro {
    pub use crate::cast_ptr;
}

pub unsafe fn raw_box_mut<T>(e: T) -> *mut T {
    Box::into_raw(Box::new(e))
}

pub unsafe fn write_pointer<T>(p: *mut gpointer, e: T) {
    p.cast::<*mut T>().write(raw_box_mut(e));
}

pub unsafe fn read_pointer<T>(p: gpointer) -> Box<T> {
    Box::from_raw(p.cast::<T>())
}

pub unsafe fn write_str(p: *mut *const gchar, s: &CString) {
    p.write(s.as_ptr().cast::<i8>());
}

pub unsafe fn read_str(p: *const gchar) -> Result<String> {
    from_cstring(CStr::from_ptr(p.cast::<i8>()))
}

pub fn from_cstring(cs: &CStr) -> Result<String> {
    cs.to_str()
        .map(|s| String::from(s))
        .map_err(|e| BackendError::map(&e, Action::Internal))
}

pub fn to_cstring(s: &str) -> Result<CString> {
    CString::new(s).map_err(|e| BackendError::map(&e, Action::Internal))
}

#[macro_export]
macro_rules! cast_ptr {
    ($var: ident, $t:ident$(<$generic:tt>)+) => {
        let $var = &*$var.cast::<$t$(<$generic>)?>();
    };
    ($var: ident, $t:ident) => {
        let $var = &*$var.cast::<$t>();
    };
}
