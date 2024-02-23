mod posix;

use io_backends::generate_backend;
use io_backends::prelude::*;

generate_backend!(posix);

#[no_mangle]
pub unsafe extern "C" fn backend_info() -> *mut JBackend {
    &mut BACKEND
}

#[cfg(test)]
mod test {
    use io_backends::prelude::*;
    use io_backends::testing::*;

    use crate::BACKEND;

    #[test]
    fn test_posix_workflow() {
        let backend: ObjectBackend = unsafe { BACKEND.anon1.object };
        let data_factory = |namespace| {
            let data = PosixData {
                file_cache: FileCache::new(),
                namespace,
            };
            Box::into_raw(Box::new(data)).cast::<gpointer>()
        };
        test_workflow(&backend, &data_factory);
    }
}
