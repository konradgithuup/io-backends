use std::{cmp::min, fs::File, io::Write, os::unix::fs::MetadataExt};

use log::{debug, trace};

use io_backends::prelude::*;
use memmap2::{MmapMut, MmapOptions, RemapOptions};

const DEFAULT_MAP_SIZE: u64 = u64::pow(2, 20);

pub struct MmapObject {
    file: File,
    mmap: MmapMut,
    size: u64,
}

impl MmapObject {
    fn enlarge(&mut self) {
        debug!(
            "resizing memory map {} b => {} b",
            self.mmap.len(),
            self.size * 2
        );
        unsafe {
            let _ = self
                .mmap
                .remap((self.size * 2) as usize, RemapOptions::new().may_move(true));
        }
    }
}

impl BackendObject for MmapObject {
    fn new(file: File) -> Result<Self> {
        let file_size = file.metadata()?.len();
        let mmap_size = u64::max(DEFAULT_MAP_SIZE, file_size);
        let mmap = unsafe {
            MmapOptions::new()
                .offset(0)
                .len(mmap_size as usize)
                .map_mut(&file)
                .map_err(|e| BackendError::map(&e, Action::Init))
        }?;

        Ok(MmapObject {
            file,
            mmap,
            size: file_size,
        })
    }

    fn read(&self, mut buffer: &mut [u8], offset: u64, length: u64) -> Result<u64> {
        if offset >= self.size {
            return Ok(0);
        }

        let offset = offset;
        let end = min(self.size, offset + length) as usize;

        let n_read = buffer.write(&self.mmap[offset as usize..end])?;

        Ok(n_read as u64)
    }

    fn write(&mut self, buffer: &[u8], offset: u64, length: u64) -> Result<u64> {
        let calc_size = offset + length;
        if self.size < calc_size {
            self.size = calc_size;
            self.file.set_len(self.size as u64)?;
        }

        if self.mmap.len() <= calc_size as usize {
            trace!(
                "Calculated size exceeds memory map: {} b <= {calc_size} b",
                self.mmap.len()
            );
            self.enlarge();
        }

        let n_written = (&mut self.mmap[offset as _..calc_size as _]).write(buffer)?;

        Ok(n_written as u64)
    }

    fn sync(&mut self) -> Result<()> {
        self.mmap
            .flush()
            .map_err(|e| BackendError::map(&e, Action::Sync))
    }

    fn status(&self) -> Result<(i64, u64)> {
        Ok((self.file.metadata()?.atime(), self.size))
    }
}

pub struct Adapter {}

impl JuleaAdapter<MmapObject> for Adapter {}
