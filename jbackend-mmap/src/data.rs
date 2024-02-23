use std::{cmp::max, fs::File, io::Write, ops::DerefMut};

const MAP_SIZE: usize = usize::pow(2, 20);

use log::error;
use memmap2::{MmapMut, MmapOptions};

use io_backends::prelude::*;

pub struct MmapData {
    pub file_cache: FileCache<MmapFile>,
    pub namespace: String,
}

pub struct MmapFile {
    pub file: File,
    pub mmap: MmapMut,
    pub size: usize,
}

impl MmapFile {
    pub fn new(file: File) -> Result<Self> {
        let file_size = file.metadata()?.len() as usize;
        let map_size = max(MAP_SIZE, file_size);
        let mmap = unsafe { MmapOptions::new().offset(0).len(map_size).map_mut(&file) }
            .map_err(|e| BackendError::map(&e, Action::Init))?;

        Ok(MmapFile {
            file,
            mmap,
            size: file_size,
        })
    }

    pub fn write(&mut self, buf: &[u8], offset: usize, len: usize) -> Result<usize> {
        let calc_size = offset + len;
        if self.size < calc_size {
            self.size = calc_size;
            self.file.set_len(self.size as u64)?;
        }

        error!("file size {}", self.file.metadata()?.len());
        (&mut self.mmap.deref_mut()[offset..calc_size]).write_all(buf)?;
        Ok(len)
    }
}
