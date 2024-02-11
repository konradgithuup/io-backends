use std::{
    collections::HashMap,
    error::Error,
    ffi::{CStr, CString},
    fmt::Display,
    fs::{self, File, OpenOptions, ReadDir},
    io::{ErrorKind, Write},
    os::{
        fd::{AsFd, AsRawFd, RawFd},
        unix::fs::{FileExt, MetadataExt},
    },
    path::PathBuf,
    ptr, slice,
    sync::{Arc, RwLock},
};

use log::{error, info};

use crate::{
    bindings::{gboolean, gchar, gconstpointer, gint64, gpointer, guint64},
    gbool::{FALSE, TRUE},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

type Bytes = u64;
type Seconds = i64;

pub struct FileCache {
    files: Arc<RwLock<HashMap<i32, File>>>,
}

impl FileCache {
    pub fn new() -> Self {
        info!("Initializing new file cache");
        FileCache {
            files: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn execute_on<T>(
        &self,
        runnable: &dyn Fn(&File) -> std::io::Result<T>,
        raw_fd: RawFd,
    ) -> std::io::Result<T> {
        match self.files.read() {
            Ok(lock) => {
                let f = lock.get(&raw_fd);
                match f {
                    Some(f) => runnable(f),
                    None => Err(create_error(
                        "Cannot execute action on file. The file is not present in the cache.",
                    )),
                }
            }
            Err(e) => {
                handle_error(e, Action::Internal);
                Err(create_error(
                    "An internal error prevents the action from being executed on the file.",
                ))
            }
        }
    }

    fn execute_mut_on<T>(
        &self,
        runnable: &dyn Fn(&mut File) -> std::io::Result<T>,
        raw_fd: RawFd,
    ) -> std::io::Result<T> {
        match self.files.write() {
            Ok(mut lock) => {
                let f: Option<&mut File> = lock.get_mut(&raw_fd);
                match f {
                    Some(f) => runnable(f),
                    None => Err(create_error(
                        "Cannot execute action on file. The file is not present in the cache.",
                    )),
                }
            }
            Err(e) => {
                handle_error(e, Action::Internal);
                Err(create_error(
                    "An internal error prevents the action from being executed on the file.",
                ))
            }
        }
    }

    #[allow(dead_code)]
    pub fn contains(&self, raw_fd: RawFd) -> bool {
        match self.files.read() {
            Ok(lock) => lock.contains_key(&raw_fd),
            Err(e) => {
                handle_error(e, Action::Internal);
                false
            }
        }
    }

    fn insert(&self, file: File) -> std::io::Result<()> {
        match self.files.write() {
            Ok(mut lock) => {
                if lock.contains_key(&file.as_raw_fd()) {
                    return Err(create_error("Cannot insert file into cache. The file descriptor is already present in the cache."));
                }
                lock.insert(file.as_fd().as_raw_fd(), file);
                Ok(())
            }
            Err(e) => {
                handle_error(&e, Action::Internal);
                Err(create_error(
                    "An internal error prevents the file from being inserted into the cache.",
                ))
            }
        }
    }

    fn remove(&self, raw_fd: RawFd) -> std::io::Result<File> {
        match self.files.write() {
            Ok(mut lock) => {
                return match lock.remove(&raw_fd) {
                    Some(f) => Ok(f),
                    None => {
                        Err(create_error("Cannot remove file from cache. The file descriptor is not present in the cache."))
                    }
                };
            }
            Err(e) => {
                handle_error(e, Action::Internal);
                Err(create_error(
                    "An internal error prevents the file from being removed from the cache.",
                ))
            }
        }
    }
}

pub struct BackendData {
    pub file_cache: FileCache,
    pub namespace: String,
}

impl BackendData {
    pub fn contains(&self, raw_fd: RawFd) -> bool {
        return self.file_cache.contains(raw_fd);
    }

    pub fn check_namespace(&self, expected: &str) -> bool {
        return self.namespace.as_str() == expected;
    }
}

pub struct BackendObject {
    pub raw_fd: RawFd,
    pub path: PathBuf,
}

pub struct BackendIterator {
    pub iter: ReadDir,
    pub prefix: Option<String>,
    pub current_name: CString,
}

// INIT

pub unsafe extern "C" fn j_init(path: *const gchar, backend_data: *mut gpointer) -> gboolean {
    finish(Action::Init, backend_init(path), backend_data)
}

unsafe fn backend_init(path: *const gchar) -> Result<BackendData> {
    let path = CStr::from_ptr(path).to_str()?;
    info!("Initializing backend in namespace {path}");

    Ok(BackendData {
        file_cache: FileCache::new(),
        namespace: String::from(path),
    })
}

// FINI

pub unsafe extern "C" fn j_fini(backend_data: gpointer) {
    // though unnecessary, 'drop' makes it easier to understand, I think...
    info!("Releasing file cache");
    drop(Box::from_raw(backend_data.cast::<BackendObject>()));
}

// CREATE

pub unsafe extern "C" fn j_create(
    backend_data: gpointer,
    namespace: *const gchar,
    path: *const gchar,
    backend_object: *mut gpointer,
) -> gboolean {
    let backend_data = &*backend_data.cast::<BackendData>();

    finish(
        Action::Create,
        backend_create(backend_data, namespace, path),
        backend_object,
    )
}

unsafe fn backend_create(
    backend_data: &BackendData,
    namespace: *const gchar,
    path: *const gchar,
) -> Result<BackendObject> {
    let path: PathBuf = build_path(backend_data, Vec::from([namespace, path]))?;

    let f: File = OpenOptions::new()
        .read(true)
        .append(true)
        .create_new(true)
        .open(&path)?;
    let fd = f.as_raw_fd();

    backend_data.file_cache.insert(f)?;

    Ok(BackendObject { raw_fd: fd, path })
}

// OPEN

pub unsafe extern "C" fn j_open(
    backend_data: gpointer,
    namespace: *const gchar,
    path: *const gchar,
    backend_object: *mut gpointer,
) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();

    finish(
        Action::Open,
        backend_open(backend_data, namespace, path),
        backend_object,
    )
}

unsafe fn backend_open(
    backend_data: &BackendData,
    namespace: *const gchar,
    path: *const gchar,
) -> Result<BackendObject> {
    let path = build_path(backend_data, Vec::from([namespace, path]))?;

    let f: File = OpenOptions::new().read(true).append(true).open(&path)?;
    let fd = f.as_raw_fd();
    backend_data.file_cache.insert(f)?;
    Ok(BackendObject { raw_fd: fd, path })
}

// DELETE

pub unsafe extern "C" fn j_delete(backend_data: gpointer, backend_object: gpointer) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let backend_object: &BackendObject = &*backend_object.cast();

    match backend_delete(&backend_data, &backend_object) {
        Ok(_) => TRUE,
        Err(e) => handle_error(e, Action::Delete),
    }
}

unsafe fn backend_delete(
    backend_data: &BackendData,
    backend_object: &BackendObject,
) -> std::io::Result<()> {
    backend_data.file_cache.remove(backend_object.raw_fd)?;
    fs::remove_file(&backend_object.path)
}

// CLOSE

pub unsafe extern "C" fn j_close(backend_data: gpointer, backend_object: gpointer) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let backend_object: &BackendObject = &*backend_object.cast();

    match backend_data.file_cache.remove(backend_object.raw_fd) {
        Ok(_) => TRUE,
        Err(e) => handle_error(e, Action::Close),
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
        let metadata = f.metadata()?;
        let last_modification: Seconds = metadata.mtime();
        let size: Bytes = metadata.size();

        Ok((last_modification, size))
    };

    match backend_data
        .file_cache
        .execute_on(&runnable, backend_object.raw_fd)
    {
        Ok((last_mod, s)) => {
            *modification_time = last_mod;
            *size = s;
            TRUE
        }
        Err(e) => handle_error(e, Action::Status),
    }
}

pub unsafe extern "C" fn j_sync(backend_data: gpointer, backend_object: gpointer) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast();
    let backend_object: &BackendObject = &*backend_object.cast();
    let runnable = |f: &mut File| f.flush();

    match backend_data
        .file_cache
        .execute_mut_on(&runnable, backend_object.raw_fd)
    {
        Ok(_) => TRUE,
        Err(e) => handle_error(e, Action::Sync),
    }
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

    match backend_data
        .file_cache
        .execute_on(&runnable, backend_object.raw_fd)
    {
        Ok(n_read) => {
            *bytes_read = n_read as u64;
            TRUE
        }
        Err(e) => handle_error(e, Action::Read),
    }
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

    match backend_data
        .file_cache
        .execute_mut_on(&runnable, backend_object.raw_fd)
    {
        Ok(n_written) => {
            *bytes_written = n_written as u64;
            TRUE
        }
        Err(e) => handle_error(e, Action::Write),
    }
}

pub unsafe extern "C" fn j_get_all(
    backend_data: gpointer,
    namespace: *const gchar,
    backend_iterator: *mut gpointer,
) -> gboolean {
    let backend_data: &BackendData = &*backend_data.cast::<BackendData>();

    finish(
        Action::CreateIterAll,
        backend_get_iterator(&backend_data, namespace, Option::None),
        backend_iterator,
    )
}

pub unsafe extern "C" fn j_get_by_prefix(
    backend_data: gpointer,
    namespace: *const gchar,
    prefix: *const gchar,
    backend_iterator: *mut gpointer,
) -> i32 {
    let backend_data: &BackendData = &*backend_data.cast::<BackendData>();

    finish(
        Action::CreateIterPrefix,
        backend_get_iterator(backend_data, namespace, Some(prefix)),
        backend_iterator,
    )
}

unsafe fn backend_get_iterator(
    backend_data: &BackendData,
    namespace: *const gchar,
    prefix: Option<*const gchar>,
) -> std::io::Result<BackendIterator> {
    let namespace = build_path(backend_data, Vec::from([namespace]))?;

    Ok(BackendIterator {
        iter: fs::read_dir(namespace)?,
        prefix: match prefix {
            Some(cs) => Some(convert_cstring(cs)?),
            None => None,
        },
        current_name: CString::default(),
    })
}

pub unsafe extern "C" fn j_iterate(
    _backend_data: gpointer,
    backend_iterator: gpointer,
    name: *mut *const gchar,
) -> gboolean {
    let backend_iterator: &mut BackendIterator = &mut *backend_iterator.cast();

    match backend_iterate(backend_iterator) {
        Ok(opt_name) => match opt_name {
            Some(n) => {
                backend_iterator.current_name = n;
                name.write(backend_iterator.current_name.as_ptr().cast::<i8>());
                TRUE
            }
            None => {
                info!("End of iterator reached. Releasing iterator.");
                drop(Box::from_raw(backend_iterator));
                FALSE
            }
        },
        Err(e) => {
            drop(Box::from_raw(backend_iterator));
            info!("An error occured. Releasing iterator.");
            handle_error(e, Action::Iter)
        }
    }
}

unsafe fn backend_iterate(
    backend_iterator: &mut BackendIterator,
) -> std::io::Result<Option<CString>> {
    while let Some(file) = backend_iterator.iter.next() {
        let file = file?;

        let file_name: String = String::from(
            file.file_name()
                .to_str()
                .ok_or(create_error("Unable to convert file name to UTF-8"))?,
        );

        let matching = match &backend_iterator.prefix {
            Some(prefix) => file_name.starts_with(prefix),
            None => true,
        };

        if matching {
            return Ok(Some(CString::new(file_name)?));
        }
    }

    Ok(None)
}

unsafe fn build_path(
    backend_data: &BackendData,
    appends: Vec<*const gchar>,
) -> std::io::Result<PathBuf> {
    appends.iter().map(|p| convert_cstring(*p)).fold(
        Ok(PathBuf::new().join(&backend_data.namespace)),
        |p1: std::io::Result<PathBuf>, p2: std::io::Result<String>| Ok(p1?.join(p2?)),
    )
}

unsafe fn convert_cstring(s: *const gchar) -> std::io::Result<String> {
    match CStr::from_ptr(s).to_str() {
        Ok(cs) => Ok(String::from(cs)),
        Err(e) => Err(create_error(e.to_string().as_str())),
    }
}

unsafe fn finish<T, E: Display>(
    action: Action,
    res: std::result::Result<T, E>,
    out: *mut gpointer,
) -> gboolean {
    match res {
        Ok(r) => {
            out.cast::<*mut T>().write(Box::into_raw(Box::new(r)));
            TRUE
        }
        Err(e) => {
            out.cast::<*mut T>().write(ptr::null_mut());
            handle_error(e, action)
        }
    }
}

fn handle_error<E: Display>(error: E, action: Action) -> gboolean {
    error!("Error during {action:?}: {error}");
    FALSE
}

fn create_error(s: &str) -> std::io::Error {
    std::io::Error::new(ErrorKind::Other, s)
}

#[allow(dead_code)]
#[derive(Debug)]
enum Action {
    Init,
    Fini,
    Create,
    Delete,
    Open,
    Close,
    Status,
    Sync,
    Read,
    Write,
    Iter,
    CreateIterAll,
    CreateIterPrefix,
    Internal,
}
