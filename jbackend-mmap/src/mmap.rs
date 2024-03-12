use std::{
    cmp::min,
    ffi::CString,
    fmt::Display,
    fs::{self, create_dir_all, File, OpenOptions},
    io::Write,
    ops::Deref,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::PathBuf,
    ptr, slice,
};

use log::{debug, error, info, trace};

use io_backends::prelude::*;

use super::data::MmapData;
use super::data::MmapFile;

pub unsafe extern "C" fn j_init(path: *const gchar, backend_data: *mut gpointer) -> gboolean {
    finish(backend_init(path), backend_data)
}

unsafe fn backend_init<'a>(path: *const gchar) -> Result<MmapData> {
    let path = read_str(path).map_err(|e| e.set_action(Action::Init))?;
    info!("Initializing backend in namespace {path}");

    Ok(MmapData {
        file_cache: FileCache::new(),
        namespace: path,
    })
}

// FINI

pub unsafe extern "C" fn j_fini(backend_data: gpointer) {
    // though unnecessary, 'drop' makes it easier to understand, I think...
    info!("Releasing file cache");
    drop(Box::from_raw(backend_data.cast::<MmapData>()));
}

// CREATE

pub unsafe extern "C" fn j_create(
    backend_data: gpointer,
    namespace: *const gchar,
    path: *const gchar,
    backend_object: *mut gpointer,
) -> gboolean {
    cast_ptr!(backend_data, MmapData);

    finish(
        backend_create(backend_data, namespace, path),
        backend_object,
    )
}

unsafe fn backend_create(
    backend_data: &MmapData,
    namespace: *const gchar,
    path: *const gchar,
) -> Result<BackendObject> {
    let path: PathBuf = build_path(backend_data, Vec::from([namespace, path]))
        .map_err(|e| e.set_action(Action::Create))?;

    match path.parent() {
        Some(dir) => create_dir_all(dir)?,
        None => (),
    }

    debug!("Create new file: {path:?}");

    let f: File = OpenOptions::new()
        .read(true)
        .append(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| BackendError::map(&e, Action::Create))?;
    let fd = f.as_raw_fd();

    backend_data
        .file_cache
        .insert(MmapFile::new(f)?, fd)
        .map_err(|e| e.set_action(Action::Create))?;

    Ok(BackendObject { raw_fd: fd, path })
}

// OPEN

pub unsafe extern "C" fn j_open(
    backend_data: gpointer,
    namespace: *const gchar,
    path: *const gchar,
    backend_object: *mut gpointer,
) -> gboolean {
    cast_ptr!(backend_data, MmapData);

    finish(backend_open(backend_data, namespace, path), backend_object)
}

unsafe fn backend_open(
    backend_data: &MmapData,
    namespace: *const gchar,
    path: *const gchar,
) -> Result<BackendObject> {
    let path = build_path(backend_data, Vec::from([namespace, path]))?;

    match path.parent() {
        Some(dir) => create_dir_all(dir)?,
        None => (),
    }

    debug!("Open file: {path:?}");

    let f: File = OpenOptions::new()
        .read(true)
        .append(true)
        .open(&path)
        .map_err(|e| BackendError::map(&e, Action::Open))?;
    let fd = f.as_raw_fd();
    backend_data
        .file_cache
        .insert(MmapFile::new(f)?, fd)
        .map_err(|e| e.set_action(Action::Open))?;
    Ok(BackendObject { raw_fd: fd, path })
}

// DELETE

pub unsafe extern "C" fn j_delete(backend_data: gpointer, backend_object: gpointer) -> gboolean {
    cast_ptr!(backend_data, MmapData);
    cast_ptr!(backend_object, BackendObject);

    match backend_delete(&backend_data, &backend_object) {
        Ok(_) => TRUE,
        Err(e) => handle_error(e),
    }
}

unsafe fn backend_delete(backend_data: &MmapData, backend_object: &BackendObject) -> Result<()> {
    backend_data
        .file_cache
        .remove(backend_object.raw_fd)
        .map_err(|e| e.set_action(Action::Delete))?;

    debug!("Deleting {}...", &backend_object.path.to_str().unwrap());

    fs::remove_file(&backend_object.path).map_err(|e| BackendError::map(&e, Action::Delete))?;

    Ok(())
}

// CLOSE

pub unsafe extern "C" fn j_close(backend_data: gpointer, backend_object: gpointer) -> gboolean {
    cast_ptr!(backend_data, MmapData);
    cast_ptr!(backend_object, BackendObject);

    match backend_data.file_cache.remove(backend_object.raw_fd) {
        Ok(_) => TRUE,
        Err(e) => handle_error(e.set_action(Action::Close)),
    }
}

pub unsafe extern "C" fn j_status(
    backend_data: gpointer,
    backend_object: gpointer,
    modification_time: *mut gint64,
    size: *mut guint64,
) -> gboolean {
    cast_ptr!(backend_data, MmapData);
    cast_ptr!(backend_object, BackendObject);

    let runnable = |bf: &MmapFile| {
        let metadata = bf.file.metadata()?;
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
        Err(e) => handle_error(e.set_action(Action::Status)),
    }
}

pub unsafe extern "C" fn j_sync(backend_data: gpointer, backend_object: gpointer) -> gboolean {
    cast_ptr!(backend_data, MmapData);
    cast_ptr!(backend_object, BackendObject);

    let runnable = |bf: &mut MmapFile| {
        bf.mmap
            .flush()
            .map_err(|e| BackendError::map(&e, Action::Sync))
    };
    match backend_data
        .file_cache
        .execute_mut_on(&runnable, backend_object.raw_fd)
    {
        Ok(_) => TRUE,
        Err(e) => handle_error(e.set_action(Action::Sync)),
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
    cast_ptr!(backend_data, MmapData);
    cast_ptr!(backend_object, BackendObject);
    trace!(
        "Read {}b at {} from {}",
        length,
        offset,
        &backend_object.path.to_str().unwrap()
    );

    let buffer = buffer.cast::<u8>();
    let runnable = |bf: &MmapFile| {
        let mut buf = slice::from_raw_parts_mut(buffer, length as usize);
        let offset = offset as usize;

        let end = min(bf.size, offset + length as usize);

        buf.write_all(&bf.mmap.deref()[offset..end])?;
        Ok(end - offset)
    };

    match backend_data
        .file_cache
        .execute_on(&runnable, backend_object.raw_fd)
    {
        Ok(n_read) => {
            *bytes_read = n_read as u64;
            TRUE
        }
        Err(e) => handle_error(e.set_action(Action::Read)),
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
    cast_ptr!(backend_data, MmapData);
    cast_ptr!(backend_object, BackendObject);
    trace!(
        "Write {} b at {} to {}",
        length,
        offset,
        &backend_object.path.to_str().unwrap()
    );

    let buffer = buffer.cast::<u8>();
    let runnable = |bf: &mut MmapFile| {
        let offset = offset as usize;
        let buf = slice::from_raw_parts(buffer, length as usize);
        if buf.len() <= offset {
            return Ok(0);
        }

        let r = bf.write(buf, offset, length as _);
        trace!("Write done: {r:?}");
        r
    };

    match backend_data
        .file_cache
        .execute_mut_on(&runnable, backend_object.raw_fd)
    {
        Ok(n_written) => {
            *bytes_written += n_written as u64;
            TRUE
        }
        Err(e) => handle_error(e.set_action(Action::Write)),
    }
}

pub unsafe extern "C" fn j_get_all(
    backend_data: gpointer,
    namespace: *const gchar,
    backend_iterator: *mut gpointer,
) -> gboolean {
    cast_ptr!(backend_data, MmapData);

    finish(
        backend_get_iterator(&backend_data, namespace, Option::None)
            .map_err(|e| e.set_action(Action::CreateIterAll)),
        backend_iterator,
    )
}

pub unsafe extern "C" fn j_get_by_prefix(
    backend_data: gpointer,
    namespace: *const gchar,
    prefix: *const gchar,
    backend_iterator: *mut gpointer,
) -> i32 {
    cast_ptr!(backend_data, MmapData);

    finish(
        backend_get_iterator(backend_data, namespace, Some(prefix))
            .map_err(|e| e.set_action(Action::CreateIterPrefix)),
        backend_iterator,
    )
}

unsafe fn backend_get_iterator(
    backend_data: &MmapData,
    namespace: *const gchar,
    prefix: Option<*const gchar>,
) -> Result<BackendIterator> {
    let namespace = build_path(backend_data, Vec::from([namespace]))?;

    Ok(BackendIterator {
        iter: fs::read_dir(namespace)?,
        prefix: match prefix {
            Some(cs) => Some(read_str(cs)?),
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
            handle_error(e.set_action(Action::Iter))
        }
    }
}

unsafe fn backend_iterate(backend_iterator: &mut BackendIterator) -> Result<Option<CString>> {
    while let Some(file) = backend_iterator.iter.next() {
        let file = file?;

        let file_name: String = String::from(file.file_name().to_str().ok_or(
            BackendError::new("Unable to convert file name to UTF-8", Action::Iter),
        )?);

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

unsafe fn build_path(backend_data: &MmapData, appends: Vec<*const gchar>) -> Result<PathBuf> {
    appends.iter().map(|p| read_str(*p)).fold(
        Ok(PathBuf::new().join(&backend_data.namespace)),
        |p1: Result<PathBuf>, p2: Result<String>| Ok(p1?.join(p2?)),
    )
}

unsafe fn finish<T, E: Display>(res: std::result::Result<T, E>, out: *mut gpointer) -> gboolean {
    match res {
        Ok(r) => {
            out.cast::<*mut T>().write(Box::into_raw(Box::new(r)));
            TRUE
        }
        Err(e) => {
            out.cast::<*mut T>().write(ptr::null_mut());
            handle_error(e)
        }
    }
}

fn handle_error<E: Display>(error: E) -> gboolean {
    error!("{error}");
    FALSE
}
