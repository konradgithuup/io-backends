use std::ffi::CStr;
use std::os::raw::c_void;
use std::path::PathBuf;

use common::{setup, TEST_PATH};
use io_backends::bindings::{gpointer, JBackend__bindgen_ty_1__bindgen_ty_1 as ObjectBackend};
use io_backends::gbool::{FALSE, TRUE};
use io_backends::posix::{BackendData, BackendObject, FileCache};
use io_backends::{get_backend, Backend};
use log::{error, info, warn};

mod common;

#[test]
fn test_workflow() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("io_backends", log::LevelFilter::Warn)
        .init();

    setup();

    let backend: ObjectBackend = get_backend(Backend::Posix);
    let backend_data: *mut gpointer = Box::into_raw(Box::new(BackendData {
        file_cache: FileCache::new(),
        namespace: String::new(),
    }))
    .cast::<gpointer>();

    let read_file: *mut gpointer = Box::into_raw(Box::new(BackendObject {
        raw_fd: 0,
        path: PathBuf::new(),
    }))
    .cast::<gpointer>();

    unsafe {
        test_init(&backend, backend_data);
        test_open(&backend, *backend_data, read_file);
        test_read(&backend, *backend_data, *read_file);
        test_status(&backend, *backend_data, *read_file);
    }
}

fn test_init(backend: &ObjectBackend, backend_data: *mut gpointer) {
    if !test_implemented("backend_init", &backend.backend_init) {
        return;
    }

    let mut s = String::from(TEST_PATH);
    s.push('\0');

    unsafe {
        handle(
            "initialize",
            assert_eq(
                backend.backend_init.unwrap()(s.as_str().as_ptr().cast::<i8>(), backend_data),
                TRUE,
                "return value",
            ),
        );
    }
}

fn test_open(backend: &ObjectBackend, backend_data: gpointer, backend_object: *mut gpointer) {
    if !test_implemented("backend_open", &backend.backend_open) {
        return;
    }

    unsafe {
        let ret = backend.backend_open.unwrap()(
            backend_data,
            "\0".as_ptr().cast::<i8>(),
            "read.txt\0".as_ptr().cast::<i8>(),
            backend_object,
        );
        handle("open", assert_eq(ret, TRUE, "return value"));
    }
}

fn test_read(backend: &ObjectBackend, backend_data: gpointer, backend_object: gpointer) {
    if !test_implemented("backend_read", &backend.backend_read) {
        return;
    }

    handle(
        "read_beginning",
        read(
            backend,
            backend_data,
            backend_object,
            TRUE,
            "Hello",
            5,
            5,
            0,
        ),
    );

    handle(
        "read_middle",
        read(
            backend,
            backend_data,
            backend_object,
            TRUE,
            "lo, w",
            5,
            5,
            3,
        ),
    );

    handle(
        "read_end",
        read(
            backend,
            backend_data,
            backend_object,
            TRUE,
            "world!",
            6,
            6,
            7,
        ),
    );

    handle(
        "overflowing_length",
        read(
            backend,
            backend_data,
            backend_object,
            TRUE,
            "Hello, world!",
            13,
            20,
            0,
        ),
    );
}

fn test_status(backend: &ObjectBackend, backend_data: gpointer, backend_object: gpointer) {
    if !test_implemented("backend_status", &backend.backend_status) {
        return;
    }

    let modification_time = Box::into_raw(Box::new(0i64));
    let size = Box::into_raw(Box::new(0u64));

    unsafe {
        let ret =
            backend.backend_status.unwrap()(backend_data, backend_object, modification_time, size);

        let validate = || -> Result<(), String> {
            assert_eq(ret, TRUE, "return value")?;
            assert_eq(*size, 13u64, "size")
        };

        handle("check_size", validate())
    }
}

fn read(
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_object: gpointer,
    expected_outcome: i32,
    expected_content: &str,
    expected_read: u64,
    len: u64,
    offset: u64,
) -> Result<(), String> {
    let mut buffer = vec![0i8; len as usize].into_boxed_slice();
    let bytes_read = Box::into_raw(Box::new(0u64));

    unsafe {
        let actual_outcome = backend.backend_read.unwrap()(
            backend_data,
            backend_object,
            buffer.as_mut_ptr().cast::<c_void>(),
            len,
            offset,
            bytes_read,
        );

        assert_eq(actual_outcome, expected_outcome, "return value")?;

        if expected_outcome == FALSE {
            return Ok(());
        }

        let actual_len = *bytes_read.cast::<u64>();

        assert_eq(actual_len, expected_read, "#bytes read")?;

        let actual_content = CStr::from_ptr(buffer.as_mut_ptr()).to_str().unwrap();
        assert_eq(actual_content, expected_content, "read content")?;
    }

    return Ok(());
}

fn test_implemented<T>(f: &str, opt: &Option<T>) -> bool {
    info!("===== Test {f} =====");
    match opt {
        Some(_) => true,
        None => {
            warn!("\u{1F7E1} backend does not implement {f}");
            false
        }
    }
}

fn handle(title: &str, outcome: Result<(), String>) {
    match outcome {
        Ok(_) => info!("\u{1F7E2} {title}"),
        Err(e) => error!("\u{1F534} {title}: {e}"),
    }
}

fn assert_eq<T: std::cmp::PartialEq + std::fmt::Debug>(
    actual: T,
    expected: T,
    property: &str,
) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "({property}) expected {expected:?} but found {actual:?}."
        ));
    }
    Ok(())
}
