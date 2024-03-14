use std::{
    fs::File,
    io::Write,
    os::unix::fs::{FileExt, MetadataExt},
};

use io_backends::common::prelude::*;

// INIT
pub struct Adapter {}

pub struct PosixObject {
    file: File,
}

impl BackendObject for PosixObject {
    fn new(file: File) -> Result<Self> {
        return Ok(PosixObject { file });
    }

    fn read(&self, buffer: &mut [u8], offset: u64, _length: u64) -> Result<u64> {
        self.file
            .read_at(buffer, offset)
            .map_err(|e| BackendError::map(&e, Action::Read))
            .map(|n| n as u64)
    }

    fn write(&mut self, buffer: &[u8], offset: u64, _length: u64) -> Result<u64> {
        self.file
            .write_at(buffer, offset)
            .map_err(|e| BackendError::map(&e, Action::Write))
            .map(|n| n as u64)
    }

    fn sync(&mut self) -> Result<()> {
        self.file
            .flush()
            .map_err(|e| BackendError::map(&e, Action::Sync))
    }

    fn status(&self) -> Result<(i64, u64)> {
        let metadata = self.file.metadata()?;
        Ok((metadata.atime(), metadata.size()))
    }
}

impl JuleaAdapter<PosixObject> for Adapter {}
