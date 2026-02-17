//! Validation functions for JSON Pointer.

use thiserror::Error;

/// Maximum allowed pointer string length.
const MAX_POINTER_LENGTH: usize = 1024;

/// Maximum allowed path depth.
const MAX_PATH_LENGTH: usize = 256;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error("POINTER_INVALID")]
    PointerInvalid,
    #[error("POINTER_TOO_LONG")]
    PointerTooLong,
    #[error("Invalid path")]
    InvalidPath,
    #[error("Path too long")]
    PathTooLong,
    #[error("Invalid path step")]
    InvalidPathStep,
}

/// Validate a JSON Pointer string.
///
/// # Errors
///
/// Returns an error if:
/// - The pointer is non-empty but doesn't start with `/`
/// - The pointer exceeds the maximum length (1024 characters)
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::validate_json_pointer;
///
/// validate_json_pointer("").unwrap();  // Root is valid
/// validate_json_pointer("/foo/bar").unwrap();  // Valid absolute pointer
/// validate_json_pointer("foo").unwrap_err();  // Missing leading /
/// ```
pub fn validate_json_pointer(pointer: &str) -> Result<(), ValidationError> {
    if pointer.is_empty() {
        return Ok(());
    }
    if !pointer.starts_with('/') {
        return Err(ValidationError::PointerInvalid);
    }
    if pointer.len() > MAX_POINTER_LENGTH {
        return Err(ValidationError::PointerTooLong);
    }
    Ok(())
}

/// Validate a path (array of path steps).
///
/// # Errors
///
/// Returns an error if:
/// - The path exceeds the maximum length (256 steps)
/// - Any step is not a valid string
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::validate_path;
///
/// validate_path(&["foo".to_string(), "bar".to_string()]).unwrap();
/// validate_path(&(0..300).map(|i| i.to_string()).collect::<Vec<_>>()).unwrap_err();
/// ```
pub fn validate_path(path: &[String]) -> Result<(), ValidationError> {
    if path.len() > MAX_PATH_LENGTH {
        return Err(ValidationError::PathTooLong);
    }
    // All strings are valid path steps in Rust
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_pointer() {
        assert!(validate_json_pointer("").is_ok());
    }

    #[test]
    fn test_validate_absolute_pointer() {
        assert!(validate_json_pointer("/").is_ok());
        assert!(validate_json_pointer("/foo").is_ok());
        assert!(validate_json_pointer("/foo/bar").is_ok());
    }

    #[test]
    fn test_validate_relative_pointer() {
        assert!(validate_json_pointer("foo").is_err());
        assert!(validate_json_pointer("foo/bar").is_err());
    }

    #[test]
    fn test_validate_long_pointer() {
        let long_pointer = "/".to_string() + &"a".repeat(2000);
        assert!(validate_json_pointer(&long_pointer).is_err());
    }

    #[test]
    fn test_validate_short_path() {
        let path = vec!["foo".to_string(), "bar".to_string()];
        assert!(validate_path(&path).is_ok());
    }

    #[test]
    fn test_validate_long_path() {
        let path: Vec<String> = (0..300).map(|i| i.to_string()).collect();
        assert!(validate_path(&path).is_err());
    }

    #[test]
    fn test_validate_max_length_path() {
        let path: Vec<String> = (0..256).map(|i| i.to_string()).collect();
        assert!(validate_path(&path).is_ok());
    }
}
