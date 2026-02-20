pub mod fn_value;
pub mod obj_value;
#[allow(clippy::module_inception)]
pub mod value;

pub use fn_value::FnValue;
pub use obj_value::ObjValue;
pub use value::{unknown, Value};
