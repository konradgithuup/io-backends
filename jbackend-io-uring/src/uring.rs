use std::{
    cell::RefCell,
    fs::File,
    os::{
        fd::{AsRawFd, RawFd},
        unix::fs::MetadataExt,
    },
    rc::Rc,
};

use io_uring::{opcode, squeue::Flags, types, IoUring};

use io_backends::prelude::*;

pub type LocalUring = Rc<RefCell<Uring>>;

thread_local! {
    pub static THREAD_URING: LocalUring = Uring::new();
}

pub struct Uring {
    io_uring: IoUring,
    user_data: u64,
}

impl Uring {
    fn new() -> LocalUring {
        Rc::new(RefCell::new(Uring {
            io_uring: IoUring::builder().setup_sqpoll(1000).build(8).unwrap(),
            user_data: 0_u64,
        }))
    }

    fn read(&mut self, buffer: &mut [u8], fd: RawFd, offset: u64) -> Result<u64> {
        let user_data = {
            self.user_data += 1;
            self.user_data
        };

        let read_op = opcode::Read::new(types::Fd(fd), buffer.as_mut_ptr(), buffer.len() as _)
            .offset(offset)
            .build()
            .flags(Flags::IO_HARDLINK)
            .user_data(user_data);

        unsafe {
            self.io_uring
                .submission()
                .push(&read_op)
                .map_err(|e| BackendError::map(&e, Action::Read))?;
        }

        self.io_uring.submit_and_wait(1)?;

        for cqe in self.io_uring.completion().into_iter() {
            if cqe.user_data() == user_data {
                return Ok(cqe.result() as _);
            }
        }

        Err(BackendError::new(
            "Missing expected CQ entry.",
            Action::Read,
        ))
    }

    fn write(&mut self, buffer: &[u8], fd: RawFd, offset: u64) -> Result<u64> {
        let user_data = {
            self.user_data += 1;
            self.user_data
        };

        let write_op = opcode::Write::new(types::Fd(fd), buffer.as_ptr(), buffer.len() as _)
            .offset(offset)
            .build()
            .flags(Flags::IO_HARDLINK)
            .user_data(user_data);

        unsafe {
            self.io_uring
                .submission()
                .push(&write_op)
                .map_err(|e| BackendError::map(&e, Action::Write))?;
        }

        self.io_uring.submit_and_wait(1)?;

        for cqe in self.io_uring.completion().into_iter() {
            if cqe.user_data() == user_data {
                return Ok(cqe.result() as _);
            }
        }

        Err(BackendError::new(
            "Missing expected CQ entry.",
            Action::Write,
        ))
    }
}

pub struct UringObject {
    file: File,
    fd: RawFd,
}

impl BackendObject for UringObject {
    fn new(file: File) -> Result<Self> {
        let fd = file.as_raw_fd();
        Ok(UringObject { file, fd })
    }

    fn read(&self, buffer: &mut [u8], offset: u64, _length: u64) -> Result<u64> {
        THREAD_URING
            .try_with(|ring| ring.borrow_mut().read(buffer, self.fd, offset))
            .map_err(|e| BackendError::map(&e, Action::Read))?
    }

    fn write(&mut self, buffer: &[u8], offset: u64, _length: u64) -> Result<u64> {
        THREAD_URING
            .try_with(|ring| ring.borrow_mut().write(buffer, self.fd, offset))
            .map_err(|e| BackendError::map(&e, Action::Read))?
    }

    fn sync(&mut self) -> Result<()> {
        // nothing to sync in current implementation(validate?)
        Ok(())
    }

    fn status(&self) -> Result<(i64, u64)> {
        let metadata = self.file.metadata()?;
        Ok((metadata.atime(), metadata.size() as _))
    }
}

pub struct Adapter {}

impl JuleaAdapter<UringObject> for Adapter {}
