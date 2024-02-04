use std::{
    collections::HashMap,
    error::Error,
    ffi::{CStr, OsStr},
    fmt::Display,
    fs::{self, File, ReadDir},
    io::{ErrorKind, Read, Write},
    ops::{Deref, DerefMut},
    os::{
        fd::{AsFd, AsRawFd, RawFd},
        unix::fs::{FileExt, MetadataExt},
    },
    path::{Path, PathBuf},
    ptr::{self, null, null_mut},
    rc::Rc,
    slice,
    sync::{Arc, RwLock},
};

use crate::{
    bindings::{
        gboolean, gchar, gconstpointer, gint64, gpointer, guint64, j_trace_file_begin, JBackend,
        JTrace, JTraceFileOperation,
    },
    gbool::{FALSE, TRUE},
    Backend,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

type Bytes = u64;
type Seconds = i64;

struct FileCache {
    files: Arc<RwLock<HashMap<i32, File>>>,
}

impl FileCache {
    fn new() -> Self {
        FileCache {
            files: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn execute_on<T>(&self, runnable: &dyn Fn(&File) -> T, raw_fd: RawFd) -> Option<T> {
        match self.files.read() {
            Ok(lock) => {
                let f = lock.get(&raw_fd);
                Some(runnable(f?))
            }
            Err(e) => None,
        }
    }

    fn execute_mut_on<T>(&self, runnable: &dyn Fn(&mut File) -> T, raw_fd: RawFd) -> Option<T> {
        match self.files.write() {
            Ok(mut lock) => {
                let f: Option<&mut File> = lock.get_mut(&raw_fd);
                Some(runnable(f?))
            }
            Err(e) => None,
        }
    }

    fn contains(&self, raw_fd: RawFd) -> bool {
        match (self.files.read()) {
            Ok(lock) => lock.contains_key(&raw_fd),
            Err(e) => false,
        }
    }

    fn insert(&self, file: File) -> bool {
        match self.files.write() {
            Ok(mut lock) => lock.insert(file.as_fd().as_raw_fd(), file).is_none(),
            Err(e) => false,
        }
    }

    fn remove(&self, raw_fd: RawFd) -> Option<File> {
        match self.files.write() {
            Ok(mut lock) => lock.remove(&raw_fd),
            Err(e) => None,
        }
    }
}

struct BackendData {
    pub file_cache: FileCache,
    pub namespace: String,
}

struct BackendObject {
    pub raw_fd: RawFd,
    pub path: PathBuf,
}

struct BackendIterator {
    pub iter: ReadDir,
    pub prefix: Option<String>,
}

pub unsafe extern "C" fn j_init(path: *const gchar, backend_data: *mut gpointer) -> gboolean {
    finish(backend_init(path), backend_data)
}

unsafe fn backend_init(path: *const gchar) -> Result<BackendData> {
    let path = CStr::from_ptr(path).to_str()?;
    let _ = File::open(&path)?;

    Ok(BackendData {
        file_cache: FileCache::new(),
        namespace: String::from(path),
    })
}

pub unsafe extern "C" fn j_fini(backend_data: gpointer) {
    // though unnecessary, 'drop' makes it easier to understand, I think...
    drop(Box::from_raw(backend_data.cast::<BackendObject>()));
}

pub unsafe extern "C" fn j_create(
    backend_data: gpointer,
    namespace: *const gchar,
    path: *const gchar,
    backend_object: *mut gpointer,
) -> gboolean {
    let backend = &*backend_data.cast::<BackendData>();
    let path = CStr::from_ptr(path).to_str().unwrap();
    let f: File = File::open(path).unwrap();
    backend.file_cache.insert(f);
    TRUE
}

pub unsafe extern "C" fn j_open(
    backend_data: gpointer,
    namespace: *const gchar,
    path: *const gchar,
    backend_object: *mut gpointer,
) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let p: PathBuf = build_path(backend_data, namespace, path);
    let f: File = File::create(&p).unwrap();

    finish(backend_open(backend_data, p), backend_object)
}

unsafe fn backend_open(backend_data: &BackendData, path: PathBuf) -> Result<BackendObject> {
    let f: File = File::create(&path)?;

    Ok(BackendObject {
        raw_fd: f.as_raw_fd(),
        path,
    })
}

pub unsafe extern "C" fn j_delete(backend_data: gpointer, backend_object: gpointer) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let backend_object: &BackendObject = &*backend_object.cast();

    match backend_delete(&backend_data, &backend_object) {
        Ok(_) => TRUE,
        Err(_) => FALSE,
    }
}

unsafe fn backend_delete(
    backend_data: &BackendData,
    backend_object: &BackendObject,
) -> std::io::Result<()> {
    match backend_data.file_cache.remove(backend_object.raw_fd) {
        Some(f) => fs::remove_file(&backend_object.path),
        None => Err(std::io::Error::new(
            ErrorKind::Other,
            "The backend object is not cached",
        )),
    }
}

pub unsafe extern "C" fn j_close(backend_data: gpointer, backend_object: gpointer) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let backend_object: &BackendObject = &*backend_object.cast();

    match backend_data.file_cache.remove(backend_object.raw_fd) {
        Some(_) => TRUE,
        None => FALSE,
    }
}

pub unsafe extern "C" fn j_status(
    backend_data: gpointer,
    backend_object: gpointer,
    modification_time: *mut gint64,
    size: *mut guint64,
) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let backend_object: &BackendObject = &*backend_object.cast();
    let runnable = |f: &File| {
        let last_modification: Seconds = f.metadata().unwrap().mtime();
        let size: Bytes = f.metadata().unwrap().size();

        (last_modification, size)
    };

    let (last_mod, s) = backend_data
        .file_cache
        .execute_on(&runnable, backend_object.raw_fd)
        .unwrap();
    *modification_time = last_mod;
    *size = s;

    TRUE
}

pub unsafe extern "C" fn j_sync(backend_data: gpointer, backend_object: gpointer) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let backend_object: &BackendObject = &*backend_object.cast();
    let runnable = |f: &mut File| f.flush();

    backend_data
        .file_cache
        .execute_mut_on(&runnable, backend_object.raw_fd);

    TRUE
}

pub unsafe extern "C" fn j_read(
    backend_data: gpointer,
    backend_object: gpointer,
    buffer: gpointer,
    length: guint64,
    offset: guint64,
    bytes_read: *mut guint64,
) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let backend_object: &BackendObject = &*backend_object.cast();
    let buffer = buffer.cast::<u8>();
    let runnable = |f: &File| f.read_at(slice::from_raw_parts_mut(buffer, length as usize), offset);

    *bytes_read = backend_data
        .file_cache
        .execute_on(&runnable, backend_object.raw_fd)
        .unwrap()
        .unwrap() as u64;

    TRUE
}

pub unsafe extern "C" fn j_write(
    backend_data: gpointer,
    backend_object: gpointer,
    buffer: gconstpointer,
    length: guint64,
    offset: guint64,
    bytes_written: *mut guint64,
) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let backend_object: &BackendObject = &*backend_object.cast();
    let buffer = buffer.cast::<u8>();
    let runnable =
        |f: &mut File| f.write_at(slice::from_raw_parts(buffer, length as usize), offset);

    *bytes_written = backend_data
        .file_cache
        .execute_mut_on(&runnable, backend_object.raw_fd)
        .unwrap()
        .unwrap() as u64;

    TRUE
}

pub unsafe extern "C" fn j_get_all(
    backend_data: gpointer,
    namespace: *const gchar,
    backend_iterator: *mut gpointer,
) -> gboolean {
    TRUE
}

pub unsafe extern "C" fn j_get_by_prefix(
    backend_data: gpointer,
    namespace: *const gchar,
    prefix: *const gchar,
    backend_iterator: *mut gpointer,
) -> i32 {
    TRUE
}

pub unsafe extern "C" fn j_iterate(
    _backend_data: gpointer,
    backend_iterator: gpointer,
    name: *mut *const gchar,
) -> gboolean {
    let backend_iterator: &mut BackendIterator = &mut *backend_iterator.cast();

    while let Some(file) = backend_iterator.iter.next() {
        let file = file.unwrap();
        let file_name: String = String::from(file.file_name().to_str().unwrap());
        let matching = match &backend_iterator.prefix {
            Some(prefix) => file_name.starts_with(prefix),
            None => true,
        };

        if matching {
            *name = file_name.as_ptr().cast::<i8>();
            return TRUE;
        }
    }

    drop(Box::from_raw(backend_iterator));
    FALSE
}

unsafe fn build_path(
    backend_data: &BackendData,
    namespace: *const gchar,
    path: *const gchar,
) -> PathBuf {
    let path = CStr::from_ptr(path).to_str().unwrap();
    let namespace = CStr::from_ptr(namespace).to_str().unwrap();

    Path::new(backend_data.namespace.as_str())
        .join(namespace)
        .join(path)
}

unsafe fn finish<T, E: Display>(res: std::result::Result<T, E>, out: *mut gpointer) -> gboolean {
    match res {
        Ok(r) => {
            out.cast::<*mut T>().write(raw_box(r));
            TRUE
        }
        Err(e) => {
            out.cast::<*mut T>().write(ptr::null_mut());
            FALSE
        }
    }
}

unsafe fn raw_box<T>(val: T) -> *mut T {
    Box::into_raw(Box::new(val))
}
