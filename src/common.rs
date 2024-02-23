mod backend;
mod error;
mod util_c;

pub mod prelude {
    pub use crate::common::backend::*;
    pub use crate::common::error::*;
    pub use crate::common::util_c::util_macro::cast_ptr;
    pub use crate::common::util_c::*;
}
