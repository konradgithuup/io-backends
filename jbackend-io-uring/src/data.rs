use std::{
    cell::{RefCell, RefMut},
    fs::File,
    os::fd::{AsRawFd, RawFd},
    rc::Rc,
    sync::atomic::AtomicU64,
};

use io_uring::{opcode, squeue::Flags, types, IoUring};

use io_backends::prelude::*;

type UserData = u64;

pub type LocalUring = Rc<RefCell<Uring>>;

thread_local! {
    pub static THREAD_URING: LocalUring = Uring::new();
}

pub struct UringData {
    pub file_cache: FileCache<UringContext>,
    pub namespace: String,
}

pub struct Uring {
    io_uring: IoUring,
    user_data: AtomicU64,
}

impl Uring {
    pub fn new() -> LocalUring {
        Rc::new(RefCell::new(Uring {
            io_uring: IoUring::builder().setup_sqpoll(1000).build(8).unwrap(),
            user_data: AtomicU64::new(0),
        }))
    }
}

pub struct UringContext {
    pub file: File,
    pub fd: RawFd,
}

impl UringContext {
    pub fn new(file: File) -> Result<UringContext> {
        let fd = file.as_raw_fd();

        Ok(UringContext { file, fd })
    }

    pub fn read(&mut self, buf: &mut [u8], offset: u64) -> Result<UserData> {
        let user_data = THREAD_URING
            .try_with(|ring| self.async_read_internal(ring.borrow_mut(), buf, offset))
            .map_err(|e| BackendError::map(&e, Action::Internal))??;

        Ok(user_data)
    }

    fn async_read_internal(
        &mut self,
        mut ring: RefMut<Uring>,
        buf: &mut [u8],
        offset: u64,
    ) -> Result<UserData> {
        let user_data = ring
            .user_data
            .fetch_add(1, std::sync::atomic::Ordering::Acquire);

        let read_op = opcode::Read::new(types::Fd(self.fd), buf.as_mut_ptr(), buf.len() as _)
            .offset(offset)
            .build()
            .flags(Flags::IO_HARDLINK)
            .user_data(user_data);

        unsafe {
            ring.io_uring
                .submission()
                .push(&read_op)
                .map_err(|e| BackendError::map(&e, Action::Read))?;
        }

        ring.io_uring.submit_and_wait(1)?;

        for cqe in ring.io_uring.completion().into_iter() {
            if cqe.user_data() == user_data {
                return Ok(cqe.result() as _);
            }
        }

        Err(BackendError::new_internal("CQE entry not present"))
    }

    pub fn write(&mut self, buf: &[u8], offset: u64) -> Result<UserData> {
        let result = THREAD_URING
            .try_with(|ring| self.async_write_internal(ring.borrow_mut(), buf, offset))
            .map_err(|e| BackendError::map(&e, Action::Internal))??;

        Ok(result)
    }

    fn async_write_internal(
        &mut self,
        mut ring: RefMut<Uring>,
        buf: &[u8],
        offset: u64,
    ) -> Result<UserData> {
        let user_data = ring
            .user_data
            .fetch_add(1, std::sync::atomic::Ordering::Acquire);

        let write_op = opcode::Write::new(types::Fd(self.fd), buf.as_ptr(), buf.len() as _)
            .offset(offset)
            .build()
            .flags(Flags::IO_HARDLINK)
            .user_data(user_data);

        unsafe {
            ring.io_uring
                .submission()
                .push(&write_op)
                .map_err(|e| BackendError::map(&e, Action::Write))?;
        }

        ring.io_uring.submit_and_wait(1)?;

        for cqe in ring.io_uring.completion().into_iter() {
            if cqe.user_data() == user_data {
                return Ok(cqe.result() as _);
            }
        }

        Err(BackendError::new_internal("CQE entry not present"))
    }

    pub fn sync(&self) -> Result<()> {
        // TODO
        Ok(())
    }
}
