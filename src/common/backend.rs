use std::{
    collections::HashMap,
    ffi::CString,
    fs::{File, ReadDir},
    io::BufReader,
    os::fd::RawFd,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use log::{error, info};

use crate::common::prelude::*;

pub type Bytes = u64;
pub type Seconds = i64;

pub struct FileCache<Data> {
    files: Arc<RwLock<HashMap<i32, Data>>>,
}

impl<Data> FileCache<Data> {
    pub fn new() -> Self {
        info!("Initializing new file cache");
        FileCache {
            files: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn execute_on<T>(&self, runnable: &dyn Fn(&Data) -> Result<T>, key: i32) -> Result<T> {
        match self.files.read() {
            Ok(lock) => {
                let f = lock.get(&key);
                match f {
                    Some(f) => runnable(f),
                    None => Err(BackendError::new_internal(
                        "Cannot execute action on file. The file is not present in the cache.",
                    )),
                }
            }
            Err(e) => Err(BackendError::map(&e, Action::Internal)),
        }
    }

    pub fn execute_mut_on<T>(
        &self,
        runnable: &dyn Fn(&mut Data) -> Result<T>,
        key: i32,
    ) -> Result<T> {
        match self.files.write() {
            Ok(mut lock) => {
                let f: Option<&mut Data> = lock.get_mut(&key);
                match f {
                    Some(f) => runnable(f),
                    None => Err(BackendError::new_internal(
                        "Cannot execute action on file. The file is not present in the cache.",
                    )),
                }
            }
            Err(e) => Err(BackendError::map(&e, Action::Internal)),
        }
    }

    #[allow(dead_code)]
    pub fn contains(&self, key: i32) -> bool {
        match self.files.read() {
            Ok(lock) => lock.contains_key(&key),
            Err(e) => {
                error!("{}", BackendError::map(&e, Action::Internal));

                false
            }
        }
    }

    pub fn insert(&self, file: Data, key: i32) -> Result<()> {
        match self.files.write() {
            Ok(mut lock) => {
                if lock.contains_key(&key) {
                    return Err(BackendError::new_internal("Cannot insert file into cache. The file descriptor is already present in the cache."));
                }
                lock.insert(key, file);
                Ok(())
            }
            Err(e) => Err(BackendError::map(&e, Action::Internal)),
        }
    }

    pub fn remove(&self, key: i32) -> Result<Data> {
        match self.files.write() {
            Ok(mut lock) => {
                return match lock.remove(&key) {
                    Some(f) => Ok(f),
                    None => {
                        Err(BackendError::new_internal("Cannot remove file from cache. The file descriptor is not present in the cache."))
                    }
                };
            }
            Err(e) => Err(BackendError::map(&e, Action::Internal)),
        }
    }
}

pub struct PosixData {
    pub file_cache: FileCache<File>,
    pub namespace: String,
}

impl PosixData {
    pub fn contains(&self, raw_fd: RawFd) -> bool {
        self.file_cache.contains(raw_fd)
    }

    pub fn check_namespace(&self, expected: &str) -> bool {
        self.namespace.as_str() == expected
    }
}

pub struct BackendObject {
    pub raw_fd: RawFd,
    pub path: PathBuf,
}

pub struct BackendIterator {
    pub iter: ReadDir,
    pub prefix: Option<String>,
    pub current_name: CString,
}

pub struct BufferedObject<'a> {
    pub br: BufReader<&'a File>,
    pub path: PathBuf,
}
