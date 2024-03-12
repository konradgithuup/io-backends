use std::{cmp::max, fs::File, io::Write};

const MAP_SIZE: usize = usize::pow(2, 20);

use log::debug;
use memmap2::{MmapMut, MmapOptions, RemapOptions};

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

        if self.mmap.len() <= calc_size {
            debug!(
                "calculated size exceeds memory map: {} <= {calc_size}",
                self.mmap.len()
            );
            unsafe {
                self.enlarge();
            }
        }

        (&mut self.mmap[offset..calc_size]).write_all(buf)?;
        Ok(len)
    }

    unsafe fn enlarge(&mut self) {
        debug!(
            "resizing memory map {} => {}",
            self.mmap.len(),
            self.size * 2
        );
        let _ = self
            .mmap
            .remap(self.size * 2 as usize, RemapOptions::new().may_move(true));
    }
}
