#[allow(dead_code)]
#[allow(nonstandard_style)]
pub mod bindings;
pub mod common;
pub mod test_facility;

pub mod prelude {
    pub use crate::bindings::*;
    pub use crate::common::prelude::*;
    pub use crate::generate_backend;
    pub use crate::ObjectBackend;
}

pub mod testing {
    pub use crate::test_facility::prelude::*;
}

use bindings::*;
pub type ObjectBackend = JBackend__bindgen_ty_1__bindgen_ty_1;

#[macro_export]
macro_rules! generate_backend {
    ($name: ident) => {
        pub static mut BACKEND: JBackend = JBackend {
            type_: JBackendType::J_BACKEND_TYPE_OBJECT,
            component: JBackendComponent::J_BACKEND_COMPONENT_SERVER,
            data: core::ptr::null_mut(),
            anon1: JBackend__bindgen_ty_1 {
                object: ObjectBackend {
                    backend_init: Some($name::j_init),
                    backend_fini: Some($name::j_fini),
                    backend_create: Some($name::j_create),
                    backend_open: Some($name::j_open),
                    backend_delete: Some($name::j_delete),
                    backend_close: Some($name::j_close),
                    backend_status: Some($name::j_status),
                    backend_sync: Some($name::j_sync),
                    backend_read: Some($name::j_read),
                    backend_write: Some($name::j_write),
                    backend_get_all: Some($name::j_get_all),
                    backend_get_by_prefix: Some($name::j_get_by_prefix),
                    backend_iterate: Some($name::j_iterate),
                },
            },
        };
    };
}
