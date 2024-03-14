use super::io_handler::BackendObject;
use std::{
    ffi::CString,
    fmt::Display,
    fs::{self, create_dir_all, File, OpenOptions},
    os::fd::{AsRawFd, RawFd},
    path::{Path, PathBuf},
    ptr, slice,
};

use log::{debug, error, info, trace};

use crate::prelude::*;

pub struct ObjectHandle {
    pub raw_fd: RawFd,
    pub path: PathBuf,
}

pub trait JuleaAdapter<T: BackendObject> {
    // INIT
    unsafe extern "C" fn j_init(path: *const gchar, backend_data: *mut gpointer) -> gboolean {
        finish(Self::backend_init(path), backend_data)
    }

    unsafe fn backend_init(path: *const gchar) -> Result<Backend<T>> {
        let path = read_str(path).map_err(|e| e.set_action(Action::Init))?;
        info!("Initializing backend in namespace {path}");

        if !Path::new(path.as_str()).is_dir() {
            trace!("Creating namespace directory");
            create_dir_all(path.as_str())?;
        }

        Ok(Backend::new(path))
    }

    // FINI
    unsafe extern "C" fn j_fini(backend_data: gpointer) {
        // though unnecessary, 'drop' makes it easier to understand, I think...
        info!("Releasing backend");
        drop(Box::from_raw(backend_data.cast::<Backend<T>>()));
    }

    // CREATE
    unsafe extern "C" fn j_create(
        backend_data: gpointer,
        namespace: *const gchar,
        path: *const gchar,
        backend_object: *mut gpointer,
    ) -> gboolean {
        cast_ptr!(backend_data, Backend<T>);

        finish(
            Self::backend_create(backend_data, namespace, path),
            backend_object,
        )
    }

    unsafe fn backend_create(
        backend_data: &Backend<T>,
        namespace: *const gchar,
        path: *const gchar,
    ) -> Result<ObjectHandle> {
        let path: PathBuf = Self::build_path(backend_data, Vec::from([namespace, path]))
            .map_err(|e| e.set_action(Action::Create))?;

        match path.parent() {
            Some(dir) => create_dir_all(dir)?,
            None => (),
        }

        debug!("Create new file: {path:?}");

        // setting O_APPEND will cause the posix backend to break: https://bugzilla.kernel.org/show_bug.cgi?id=43178
        let f: File = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| BackendError::map(&e, Action::Create))?;
        let fd = f.as_raw_fd();

        let handle: T = T::new(f)?;

        backend_data
            .object_store
            .insert(handle, fd)
            .map_err(|e| e.set_action(Action::Create))?;

        Ok(ObjectHandle { raw_fd: fd, path })
    }

    //
    unsafe extern "C" fn j_open(
        backend_data: gpointer,
        namespace: *const gchar,
        path: *const gchar,
        backend_object: *mut gpointer,
    ) -> gboolean {
        cast_ptr!(backend_data, Backend<T>);

        finish(
            Self::backend_open(backend_data, namespace, path),
            backend_object,
        )
    }

    // OPEN
    unsafe fn backend_open(
        backend_data: &Backend<T>,
        namespace: *const gchar,
        path: *const gchar,
    ) -> Result<ObjectHandle> {
        let path = Self::build_path(backend_data, Vec::from([namespace, path]))?;

        debug!("Open path: {path:?}");

        let f: File = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| BackendError::map(&e, Action::Open))?;
        let fd = f.as_raw_fd();

        let handle: T = T::new(f)?;

        backend_data
            .object_store
            .insert(handle, fd)
            .map_err(|e| e.set_action(Action::Open))?;
        Ok(ObjectHandle { raw_fd: fd, path })
    }

    // DELETE
    unsafe extern "C" fn j_delete(backend_data: gpointer, backend_object: gpointer) -> gboolean {
        cast_ptr!(backend_data, Backend<T>);
        cast_ptr!(backend_object, ObjectHandle);

        match Self::backend_delete(&backend_data, &backend_object) {
            Ok(_) => TRUE,
            Err(e) => handle_error(e),
        }
    }

    unsafe fn backend_delete(
        backend_data: &Backend<T>,
        backend_object: &ObjectHandle,
    ) -> Result<()> {
        backend_data
            .object_store
            .remove(backend_object.raw_fd)
            .map_err(|e| e.set_action(Action::Delete))?;
        Ok(fs::remove_file(&backend_object.path)
            .map_err(|e| BackendError::map(&e, Action::Delete))?)
    }

    // CLOSE
    unsafe extern "C" fn j_close(backend_data: gpointer, backend_object: gpointer) -> gboolean {
        cast_ptr!(backend_data, Backend<T>);
        cast_ptr!(backend_object, ObjectHandle);

        match backend_data.object_store.remove(backend_object.raw_fd) {
            Ok(_) => TRUE,
            Err(e) => handle_error(e.set_action(Action::Close)),
        }
    }

    // STATUS
    unsafe extern "C" fn j_status(
        backend_data: gpointer,
        backend_object: gpointer,
        modification_time: *mut gint64,
        size: *mut guint64,
    ) -> gboolean {
        cast_ptr!(backend_data, Backend<T>);
        cast_ptr!(backend_object, ObjectHandle);

        match backend_data.status(backend_object) {
            Ok((last_mod, s)) => {
                *modification_time = last_mod;
                *size = s;
                TRUE
            }
            Err(e) => handle_error(e.set_action(Action::Status)),
        }
    }

    // SYNC
    unsafe extern "C" fn j_sync(backend_data: gpointer, backend_object: gpointer) -> gboolean {
        cast_ptr!(backend_data, Backend<T>);
        cast_ptr!(backend_object, ObjectHandle);

        match backend_data.sync(backend_object) {
            Ok(_) => TRUE,
            Err(e) => handle_error(e.set_action(Action::Sync)),
        }
    }

    // READ
    unsafe extern "C" fn j_read(
        backend_data: gpointer,
        backend_object: gpointer,
        buffer: gpointer,
        length: guint64,
        offset: guint64,
        bytes_read: *mut guint64,
    ) -> gboolean {
        cast_ptr!(backend_data, Backend<T>);
        cast_ptr!(backend_object, ObjectHandle);

        let buffer = slice::from_raw_parts_mut(buffer.cast::<u8>(), length as _);

        match backend_data.read(backend_object, buffer, offset, length) {
            Ok(n_read) => {
                *bytes_read = n_read as u64;
                trace!(
                    "Read {n_read}/{length} b, offset {offset} b, path {}",
                    backend_object
                        .path
                        .to_str()
                        .unwrap_or("<cannot display path with non-UTF8 characters.>")
                );
                TRUE
            }
            Err(e) => handle_error(e.set_action(Action::Read)),
        }
    }

    // WRITE
    unsafe extern "C" fn j_write(
        backend_data: gpointer,
        backend_object: gpointer,
        buffer: gconstpointer,
        length: guint64,
        offset: guint64,
        bytes_written: *mut guint64,
    ) -> gboolean {
        cast_ptr!(backend_data, Backend<T>);
        cast_ptr!(backend_object, ObjectHandle);

        trace!(
            "Write {} b at {} to {}",
            length,
            offset,
            &backend_object.path.to_str().unwrap()
        );

        let buffer = slice::from_raw_parts(buffer.cast::<u8>(), length as _);

        match backend_data.write(backend_object, buffer, offset, length) {
            Ok(n_written) => {
                *bytes_written += n_written as u64;
                trace!(
                    "Wrote {n_written}/{length} b, offset {offset} b, path {}",
                    backend_object
                        .path
                        .to_str()
                        .unwrap_or("<cannot display path with non-UTF8 characters.>")
                );
                TRUE
            }
            Err(e) => handle_error(e.set_action(Action::Write)),
        }
    }

    unsafe extern "C" fn j_get_all(
        backend_data: gpointer,
        namespace: *const gchar,
        backend_iterator: *mut gpointer,
    ) -> gboolean {
        cast_ptr!(backend_data, Backend<T>);

        finish(
            Self::backend_get_iterator(&backend_data, namespace, Option::None)
                .map_err(|e| e.set_action(Action::CreateIterAll)),
            backend_iterator,
        )
    }

    unsafe extern "C" fn j_get_by_prefix(
        backend_data: gpointer,
        namespace: *const gchar,
        prefix: *const gchar,
        backend_iterator: *mut gpointer,
    ) -> i32 {
        cast_ptr!(backend_data, Backend<T>);

        finish(
            Self::backend_get_iterator(backend_data, namespace, Some(prefix))
                .map_err(|e| e.set_action(Action::CreateIterPrefix)),
            backend_iterator,
        )
    }

    unsafe fn backend_get_iterator(
        backend_data: &Backend<T>,
        namespace: *const gchar,
        prefix: Option<*const gchar>,
    ) -> Result<BackendIterator> {
        let namespace = Self::build_path(backend_data, Vec::from([namespace]))?;

        Ok(BackendIterator {
            iter: fs::read_dir(namespace)?,
            prefix: match prefix {
                Some(cs) => Some(read_str(cs)?),
                None => None,
            },
            current_name: CString::default(),
        })
    }

    unsafe extern "C" fn j_iterate(
        _backend_data: gpointer,
        backend_iterator: gpointer,
        name: *mut *const gchar,
    ) -> gboolean {
        let backend_iterator: &mut BackendIterator = &mut *backend_iterator.cast();

        match Self::backend_iterate(backend_iterator) {
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

    unsafe fn build_path(backend_data: &Backend<T>, appends: Vec<*const gchar>) -> Result<PathBuf> {
        appends.iter().map(|p| read_str(*p)).fold(
            Ok(PathBuf::new().join(&backend_data.namespace)),
            |p1: Result<PathBuf>, p2: Result<String>| Ok(p1?.join(p2?)),
        )
    }
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
