use std::{
    collections::HashMap,
    fs::File,
    sync::{Arc, RwLock},
};

use log::{error, info};

use crate::common::error::Result;

use super::{
    error::{Action, BackendError},
    prelude::ObjectHandle,
};

pub trait BackendObject: Sized {
    fn new(file: File) -> Result<Self>;

    fn read(&self, buffer: &mut [u8], offset: u64, length: u64) -> Result<u64>;

    fn write(&mut self, buffer: &[u8], offset: u64, length: u64) -> Result<u64>;

    fn sync(&mut self) -> Result<()>;

    fn status(&self) -> Result<(i64, u64)>;
}

pub struct ObjectStore<T: BackendObject> {
    files: Arc<RwLock<HashMap<i32, T>>>,
}

impl<T: BackendObject> ObjectStore<T> {
    pub fn new() -> Self {
        info!("Initializing new object store");
        ObjectStore {
            files: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn read(&self, key: i32, buffer: &mut [u8], offset: u64, length: u64) -> Result<u64> {
        let mut read_op = |data: &mut T| data.read(buffer, offset, length);
        self.execute(key, &mut read_op)
    }

    pub fn write(&self, key: i32, buffer: &[u8], offset: u64, length: u64) -> Result<u64> {
        let mut write_op = |data: &mut T| data.write(buffer, offset, length);
        self.execute(key, &mut write_op)
    }

    pub fn status(&self, key: i32) -> Result<(i64, u64)> {
        let mut status_op = |data: &mut T| data.status();
        self.execute(key, &mut status_op)
    }

    pub fn sync(&self, key: i32) -> Result<()> {
        let mut sync_op = |data: &mut T| data.sync();
        self.execute(key, &mut sync_op)
    }

    fn execute<R>(&self, key: i32, runnable: &mut dyn FnMut(&mut T) -> Result<R>) -> Result<R> {
        runnable(
            self.files
                .write()
                .map_err(|e| BackendError::map(&e, Action::Internal))?
                .get_mut(&key)
                .ok_or(BackendError::new(
                    "Object store doesn't contain a matching object.",
                    Action::Internal,
                ))?,
        )
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

    pub fn insert(&self, file: T, key: i32) -> Result<()> {
        match self.files.write() {
            Ok(mut lock) => {
                if lock.contains_key(&key) {
                    return Err(BackendError::new_internal(
                        "Cannot insert object, object store already contains a matching object.",
                    ));
                }
                lock.insert(key, file);
                Ok(())
            }
            Err(e) => Err(BackendError::map(&e, Action::Internal)),
        }
    }

    pub fn remove(&self, key: i32) -> Result<T> {
        match self.files.write() {
            Ok(mut lock) => {
                return match lock.remove(&key) {
                    Some(f) => Ok(f),
                    None => Err(BackendError::new_internal(
                        "Cannot remove object, object store does not contain a matching object.",
                    )),
                };
            }
            Err(e) => Err(BackendError::map(&e, Action::Internal)),
        }
    }
}

pub struct Backend<T: BackendObject> {
    pub object_store: ObjectStore<T>,
    pub namespace: String,
}

impl<T: BackendObject> Backend<T> {
    pub fn new(path: String) -> Self {
        Backend {
            object_store: ObjectStore::new(),
            namespace: path,
        }
    }

    pub fn read(
        &self,
        backend_object: &ObjectHandle,
        buffer: &mut [u8],
        offset: u64,
        length: u64,
    ) -> Result<u64> {
        self.object_store
            .read(backend_object.raw_fd, buffer, offset, length)
    }

    pub fn write(
        &self,
        backend_object: &ObjectHandle,
        buffer: &[u8],
        offset: u64,
        length: u64,
    ) -> Result<u64> {
        self.object_store
            .write(backend_object.raw_fd, buffer, offset, length)
    }

    pub fn status(&self, backend_object: &ObjectHandle) -> Result<(i64, u64)> {
        self.object_store.status(backend_object.raw_fd)
    }

    pub fn sync(&self, backend_object: &ObjectHandle) -> Result<()> {
        self.object_store.sync(backend_object.raw_fd)
    }
}
