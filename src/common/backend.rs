use std::{ffi::CString, fs::ReadDir};

pub type Bytes = u64;
pub type Seconds = i64;

pub struct BackendIterator {
    pub iter: ReadDir,
    pub prefix: Option<String>,
    pub current_name: CString,
}
