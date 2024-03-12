#![allow(dead_code)]
use std::ffi::{CStr, CString};
use std::fs::{self};
use std::os::raw::c_void;
use std::path::PathBuf;

use crate::test_facility::filesystem::{
    setup, shutdown, CREATE_FILE, DELETE_FILE, READ_FILE, WRITE_FILE,
};

use assert_fs::fixture::PathChild;
use assert_fs::TempDir;

use crate::bindings::{gpointer, JBackend__bindgen_ty_1__bindgen_ty_1 as ObjectBackend};
use crate::common::prelude::{BackendIterator, BackendObject, PosixData, FALSE, TRUE};
use log::{error, info, warn};

pub fn test_workflow(backend: &ObjectBackend, data_factory: impl Fn(String) -> *mut gpointer) {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        //        .filter_module("io_backends", log::LevelFilter::Warn)
        .try_init();

    let temp = setup();

    let backend_data = data_factory(String::from(temp.to_str().unwrap()));

    let read_file: *mut gpointer = Box::into_raw(Box::new(BackendObject {
        raw_fd: 0,
        path: PathBuf::new(),
    }))
    .cast::<gpointer>();

    let write_file: *mut gpointer = Box::into_raw(Box::new(BackendObject {
        raw_fd: 0,
        path: PathBuf::new(),
    }))
    .cast::<gpointer>();

    let delete_file: *mut gpointer = Box::into_raw(Box::new(BackendObject {
        raw_fd: 0,
        path: PathBuf::new(),
    }))
    .cast::<gpointer>();

    let create_file: *mut gpointer = Box::into_raw(Box::new(BackendObject {
        raw_fd: 0,
        path: PathBuf::new(),
    }))
    .cast::<gpointer>();

    let all_iter: *mut gpointer = Box::into_raw(Box::new(BackendIterator {
        iter: fs::read_dir("./").unwrap(),
        prefix: None,
        current_name: CString::default(),
    }))
    .cast::<gpointer>();

    let prefix_iter: *mut gpointer = Box::into_raw(Box::new(BackendIterator {
        iter: fs::read_dir("./").unwrap(),
        prefix: None,
        current_name: CString::default(),
    }))
    .cast::<gpointer>();

    unsafe {
        test_init("backend_init", &backend, backend_data, &temp);

        test_open(
            "backend_open read.txt",
            &backend,
            *backend_data,
            READ_FILE,
            read_file,
        );
        test_open(
            "backend_open delete.txt",
            &backend,
            *backend_data,
            DELETE_FILE,
            delete_file,
        );
        test_open(
            "backend_open write.txt",
            &backend,
            *backend_data,
            WRITE_FILE,
            write_file,
        );

        test_create(
            "backend_create",
            &backend,
            *backend_data,
            create_file,
            &temp,
        );

        test_write_sync(
            "backend_write_to_opened",
            &backend,
            *backend_data,
            *write_file,
            WRITE_FILE,
            &temp,
        );

        test_write_sync(
            "backend_write_to_created",
            &backend,
            *backend_data,
            *create_file,
            CREATE_FILE,
            &temp,
        );

        test_read(
            "backend_newly_written",
            &backend,
            *backend_data,
            *write_file,
        );

        test_read("backend_read", &backend, *backend_data, *read_file);

        test_status("backend_status", &backend, *backend_data, *read_file);

        test_close("backend_close", &backend, *backend_data, *read_file);

        test_delete(
            "backend_delete",
            &backend,
            *backend_data,
            *delete_file,
            &temp,
        );

        test_create_iter_all("backend_create_iter_all", &backend, *backend_data, all_iter);

        test_create_iter_prefix(
            "backend_create_iter_prefix",
            &backend,
            *backend_data,
            prefix_iter,
        );

        test_iter(
            "backend_iter_all",
            &backend,
            *backend_data,
            *all_iter,
            &mut Vec::from([
                String::from("prefix_a.txt"),
                String::from("prefix_b.txt"),
                String::from("c.txt"),
            ]),
        );

        test_iter(
            "backend_iter_prefix",
            &backend,
            *backend_data,
            *prefix_iter,
            &mut Vec::from([String::from("prefix_a.txt"), String::from("prefix_b.txt")]),
        );
    }
    shutdown(temp);
}

fn test_init(title: &str, backend: &ObjectBackend, backend_data: *mut gpointer, temp: &TempDir) {
    if !test_implemented(title, &backend.backend_init) {
        return;
    }
    let mut namespace = match temp.canonicalize().unwrap().to_str() {
        Some(path) => String::from(path),
        None => panic!("Non-UTF8 characters in path to namespace"),
    };
    namespace.push('\0');
    println!("{namespace}");

    unsafe {
        handle(
            "initialize",
            assert_eq(
                backend.backend_init.unwrap()(
                    namespace.as_str().as_ptr().cast::<i8>(),
                    backend_data,
                ),
                TRUE,
                "return value",
            ),
        );
    }
}

fn test_open(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    file_name: &str,
    backend_object: *mut gpointer,
) {
    if !test_implemented(title, &backend.backend_open) {
        return;
    }

    let mut file_name = String::from(file_name);
    file_name.push('\0');

    unsafe {
        let ret = backend.backend_open.unwrap()(
            backend_data,
            "\0".as_ptr().cast::<i8>(),
            file_name.as_ptr().cast::<i8>(),
            backend_object,
        );
        handle("open", assert_eq(ret, TRUE, "return value"));
    }
}

fn test_create(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_object: *mut gpointer,
    temp: &TempDir,
) {
    if !test_implemented(title, &backend.backend_create) {
        return;
    }

    let mut file_name = String::from(CREATE_FILE);
    file_name.push('\0');

    handle(
        "precondition",
        assert_eq(
            temp.child(CREATE_FILE).exists(),
            false,
            "file does not exist",
        ),
    );

    unsafe {
        let ret = backend.backend_create.unwrap()(
            backend_data,
            "\0".as_ptr().cast::<i8>(),
            file_name.as_ptr().cast::<i8>(),
            backend_object,
        );

        handle("create", assert_eq(ret, TRUE, "return value"));
    }

    handle(
        "precondition",
        assert_eq(temp.child(CREATE_FILE).exists(), true, "file exist"),
    );
}

fn test_read(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_object: gpointer,
) {
    if !test_implemented(title, &backend.backend_read) {
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

fn test_write_sync(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_object: gpointer,
    filename: &str,
    temp: &TempDir,
) {
    if !test_implemented(title, &backend.backend_write) {
        return;
    }

    handle(
        "precondition",
        assert_eq(
            fs::read_to_string(temp.child(filename).to_path_buf()).is_ok_and(|c| c.is_empty()),
            true,
            "empty file",
        ),
    );

    let test_input: &str = "Hello, world!";

    unsafe {
        let mut buffer = String::from(test_input)
            .as_mut_vec()
            .to_owned()
            .into_boxed_slice();
        let bytes_written = Box::into_raw(Box::new(0u64));
        let ret = backend.backend_write.unwrap()(
            backend_data,
            backend_object,
            buffer.as_mut_ptr().cast::<c_void>(),
            buffer.len() as u64,
            0,
            bytes_written,
        );

        handle(
            "write",
            assert_eq(ret, TRUE, "return value").and(assert_eq(
                *bytes_written,
                buffer.len() as u64,
                "bytes written",
            )),
        );
    }

    if !test_implemented(title, &backend.backend_sync) {
        return;
    }

    unsafe {
        let ret = backend.backend_sync.unwrap()(backend_data, backend_object);
        handle("sync", assert_eq(ret, TRUE, "return value"));
    }

    handle(
        "postcondition",
        assert_eq(
            fs::read_to_string(temp.child(filename).to_path_buf())
                .is_ok_and(|c| c.as_str() == test_input),
            true,
            "file written to",
        ),
    );
}

fn test_status(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_object: gpointer,
) {
    if !test_implemented(title, &backend.backend_status) {
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

fn test_close(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_object: gpointer,
) {
    if !test_implemented(title, &backend.backend_close) {
        return;
    }

    unsafe {
        let ret = backend.backend_close.unwrap()(backend_data, backend_object);
        handle(title, assert_eq(ret, TRUE, "return value"));
        if ret == FALSE {
            return;
        }

        let backend_data = &*backend_data.cast::<PosixData>();
        let backend_object = &*backend_object.cast::<BackendObject>();
        handle(
            "postcondition",
            assert_eq(
                backend_data.contains(backend_object.raw_fd),
                false,
                "fd still in cache",
            ),
        );
    }
}

fn test_delete(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_object: gpointer,
    temp: &TempDir,
) {
    if !test_implemented(title, &backend.backend_delete) {
        return;
    }

    let mut delete_file = String::from(DELETE_FILE);
    delete_file.push('\0');

    handle(
        "precondition",
        assert_eq(temp.child(DELETE_FILE).is_file(), true, "file exists"),
    );

    unsafe {
        let ret = backend.backend_delete.unwrap()(backend_data, backend_object);
        handle("delete", assert_eq(ret, TRUE, "return value"));
    }
    handle(
        "postcondition",
        assert_eq(
            temp.child(DELETE_FILE).is_file(),
            false,
            "file does not exist",
        ),
    );
}

fn test_create_iter_all(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_iterator: *mut gpointer,
) {
    if !test_implemented(title, &backend.backend_get_all) {
        return;
    }

    unsafe {
        let ret = backend.backend_get_all.unwrap()(
            backend_data,
            "subdir\0".as_ptr().cast::<i8>(),
            backend_iterator,
        );
        handle("create", assert_eq(ret, TRUE, "return value"));
    }
}

fn test_create_iter_prefix(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_iterator: *mut gpointer,
) {
    if !test_implemented(title, &backend.backend_get_by_prefix) {
        return;
    }

    unsafe {
        let ret = backend.backend_get_by_prefix.unwrap()(
            backend_data,
            "subdir\0".as_ptr().cast::<i8>(),
            "prefix\0".as_ptr().cast::<i8>(),
            backend_iterator,
        );
        handle("create", assert_eq(ret, TRUE, "return value"));
    }
}

fn test_iter(
    title: &str,
    backend: &ObjectBackend,
    backend_data: gpointer,
    backend_iterator: gpointer,
    expected_filenames: &mut Vec<String>,
) {
    if !test_implemented(title, &backend.backend_iterate) {
        return;
    }

    unsafe {
        let file_name = Box::into_raw(Box::new("".as_ptr().cast::<i8>()));
        for _ in expected_filenames.iter() {
            let ret = backend.backend_iterate.unwrap()(backend_data, backend_iterator, file_name);
            handle("find next", assert_eq(ret, TRUE, "return value"));
            if ret == FALSE {
                return;
            }

            let actual_name =
                String::from(CStr::from_ptr(*file_name).to_str().unwrap_or_else(|e| {
                    error!("UTF-8 Error (valid until: {}", e.valid_up_to());
                    error!("{}", CStr::from_ptr(*file_name).to_string_lossy());
                    panic!("AAAAAAAAAH");
                }));
            handle(
                "name expected",
                assert_eq(expected_filenames.contains(&actual_name), true, "contains"),
            )
        }

        let ret = backend.backend_iterate.unwrap()(backend_data, backend_iterator, file_name);
        handle("iterator done", assert_eq(ret, FALSE, "return value"));
    }
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
