mod uring;

use io_backends::generate_backend;
use io_backends::prelude::*;
use log::debug;
use log::info;

generate_backend!(uring);

#[no_mangle]
pub unsafe extern "C" fn backend_info() -> *mut JBackend {
    match init_logger() {
        Ok(_) => info!("logger initialized."),
        Err(e) => {
            let _ = println!("Error while initializing logger: {e:?}");
        }
    };
    debug!("backend info called");
    &mut BACKEND
}

#[cfg(test)]
mod test {
    use io_backends::prelude::*;
    use io_backends::testing::*;

    use crate::uring::UringObject;
    use crate::BACKEND;

    #[test]
    fn test_mmap_workflow() {
        let backend: ObjectBackend = unsafe { BACKEND.anon1.object };
        let data_factory = |namespace| {
            let data = Backend::<UringObject> {
                object_store: ObjectStore::new(),
                namespace,
            };
            Box::into_raw(Box::new(data)).cast::<gpointer>()
        };
        test_workflow(&backend, &data_factory);
    }
}
