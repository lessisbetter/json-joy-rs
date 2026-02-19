pub mod types;
pub mod validator;

pub use types::{ErrorMode, ValidationResult, ValidatorOptions};
pub use validator::validate;
