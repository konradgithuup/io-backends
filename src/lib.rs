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
            flags: JBackendFlags::J_BACKEND_FLAGS_DO_NOT_UNLOAD,
            anon1: JBackend__bindgen_ty_1 {
                object: ObjectBackend {
                    backend_init: Some($name::Adapter::j_init),
                    backend_fini: Some($name::Adapter::j_fini),
                    backend_create: Some($name::Adapter::j_create),
                    backend_open: Some($name::Adapter::j_open),
                    backend_delete: Some($name::Adapter::j_delete),
                    backend_close: Some($name::Adapter::j_close),
                    backend_status: Some($name::Adapter::j_status),
                    backend_sync: Some($name::Adapter::j_sync),
                    backend_read: Some($name::Adapter::j_read),
                    backend_write: Some($name::Adapter::j_write),
                    backend_get_all: Some($name::Adapter::j_get_all),
                    backend_get_by_prefix: Some($name::Adapter::j_get_by_prefix),
                    backend_iterate: Some($name::Adapter::j_iterate),
                },
            },
        };
    };
}
