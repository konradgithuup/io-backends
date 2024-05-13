mod posix;

use io_backends::generate_backend;
use io_backends::prelude::*;
use log::debug;
use log::trace;

generate_backend!(posix);

#[no_mangle]
pub unsafe extern "C" fn backend_info() -> *mut JBackend {
    match init_logger() {
        Ok(_) => debug!("Logger initialized."),
        Err(e) => {
            let _ = println!("Error while initializing logger: {e:?}");
        }
    };
    trace!("backend_info() called.");
    &mut BACKEND
}

#[cfg(test)]
mod test {
    use io_backends::prelude::*;
    use io_backends::test_facility::writes;
    use io_backends::testing::*;

    use crate::posix::PosixObject;
    use crate::BACKEND;

    // #[test]
    fn _test_posix_workflow() {
        let backend: ObjectBackend = unsafe { BACKEND.anon1.object };
        let data_factory = |namespace| {
            let data = Backend::<PosixObject> {
                object_store: ObjectStore::new(),
                namespace,
            };
            Box::into_raw(Box::new(data)).cast::<gpointer>()
        };
        test_workflow(&backend, &data_factory);
    }

    #[test]
    fn repeated_writes() {
        let backend: ObjectBackend = unsafe { BACKEND.anon1.object };
        let data_factory = |namespace| {
            let data = Backend::<PosixObject> {
                object_store: ObjectStore::new(),
                namespace,
            };
            Box::into_raw(Box::new(data)).cast::<gpointer>()
        };

        writes::test_writes(&backend, data_factory)
    }
}
