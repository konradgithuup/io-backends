mod data;
mod uring;

use io_backends::generate_backend;
use io_backends::prelude::*;

generate_backend!(uring);

#[no_mangle]
pub unsafe extern "C" fn backend_info() -> *mut JBackend {
    &mut BACKEND
}

#[cfg(test)]
mod test {
    use crate::data::*;
    use io_backends::prelude::*;
    use io_backends::testing::*;

    use crate::BACKEND;

    #[test]
    fn test_mmap_workflow() {
        let backend: ObjectBackend = unsafe { BACKEND.anon1.object };
        let data_factory = |namespace| {
            let data = UringData {
                file_cache: FileCache::new(),
                namespace,
            };
            Box::into_raw(Box::new(data)).cast::<gpointer>()
        };
        test_workflow(&backend, &data_factory);
    }
}
