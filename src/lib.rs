use std::os::raw::c_void;

use bindings::{
    JBackend, JBackendComponent, JBackendType, JBackend__bindgen_ty_1,
    JBackend__bindgen_ty_1__bindgen_ty_1 as ObjectBackend,
};
use log::info;

#[allow(dead_code)]
#[allow(nonstandard_style)]
mod bindings;
mod gbool;
mod posix;

pub enum Backend {
    Posix,
    Buffered,
    Mmap,
    IoUring,
}

pub fn init() {
    info!("test");
}

pub fn create_backend() -> JBackend {
    JBackend {
        type_: JBackendType::J_BACKEND_TYPE_OBJECT,
        component: JBackendComponent::J_BACKEND_COMPONENT_CLIENT,
        // TODO data?
        data: 1 as *mut c_void,
        anon1: JBackend__bindgen_ty_1 {
            object: get_backend(Backend::Posix),
        },
    }
}

generate_backend!(posix);

fn get_backend(backend_type: Backend) -> ObjectBackend {
    // TODO backends
    match backend_type {
        Backend::Posix => posix(),
        Backend::Buffered => posix(),
        Backend::Mmap => posix(),
        Backend::IoUring => posix(),
    }
}

#[macro_export]
macro_rules! generate_backend {
    ($module_name: ident) => {
        fn $module_name() -> ObjectBackend {
            ObjectBackend {
                backend_init: Some($module_name::j_init),
                backend_fini: Some($module_name::j_fini),
                backend_create: Some($module_name::j_create),
                backend_open: Some($module_name::j_open),
                backend_delete: Some($module_name::j_delete),
                backend_close: Some($module_name::j_close),
                backend_status: Some($module_name::j_status),
                backend_sync: Some($module_name::j_sync),
                backend_read: Some($module_name::j_read),
                backend_write: Some($module_name::j_write),
                backend_get_all: Some($module_name::j_get_all),
                backend_get_by_prefix: Some($module_name::j_get_by_prefix),
                backend_iterate: Some($module_name::j_iterate),
            }
        }
    };
}
