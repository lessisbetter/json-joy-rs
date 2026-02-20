pub mod types;
#[allow(clippy::module_inception)]
pub mod validator;

pub use types::{ErrorMode, ValidationResult, ValidatorOptions};
pub use validator::validate;
