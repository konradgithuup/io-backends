use std::{os::raw::c_void, path::PathBuf};

use crate::{
    bindings::{gpointer, JBackend__bindgen_ty_1__bindgen_ty_1 as ObjectBackend},
    common::prelude::{ObjectHandle, FALSE},
    testing::{setup, shutdown},
};

pub fn test_writes(backend: &ObjectBackend, data_factory: impl Fn(String) -> *mut gpointer) {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        //        .filter_module("io_backends", log::LevelFilter::Warn)
        .try_init();

    let temp = setup();
    let backend_data = data_factory(String::from(temp.to_str().unwrap()));

    let file: *mut gpointer = Box::into_raw(Box::new(ObjectHandle {
        raw_fd: 0,
        path: PathBuf::new(),
    }))
    .cast::<gpointer>();

    let mut namespace = match temp.canonicalize().unwrap().to_str() {
        Some(path) => String::from(path),
        None => panic!("Non-UTF8 characters in path to namespace"),
    };
    namespace.push('\0');

    unsafe {
        if backend.backend_init.unwrap()(namespace.as_ptr().cast::<i8>(), backend_data) == FALSE {
            shutdown(temp);
            panic!("Error in init")
        }
    };

    unsafe {
        let mut path = String::from("benches");
        path.push('\0');
        let mut fname = String::from("writes");
        fname.push('\0');
        let ret = backend.backend_create.unwrap()(
            *backend_data,
            path.as_ptr().cast::<i8>(),
            fname.as_ptr().cast::<i8>(),
            file,
        );
        if ret == FALSE {
            shutdown(temp);
            panic!("Error in create")
        }
    };

    let bytes_written = Box::into_raw(Box::new(0u64));

    let s = "xyz1234567890";
    let len = s.len();
    let buffer = unsafe {
        String::from(s)
            .as_mut_vec()
            .to_owned()
            .into_boxed_slice()
            .as_mut_ptr()
            .cast::<c_void>()
    };
    unsafe {
        for _ in 0..100_000 {
            let ret = backend.backend_write.unwrap()(
                *backend_data,
                *file,
                buffer,
                0u64,
                len as _,
                bytes_written,
            );
            if ret == FALSE {
                shutdown(temp);
                panic!("Error in bench")
            }
        }
    }

    shutdown(temp)
}
