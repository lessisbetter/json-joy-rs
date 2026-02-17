use std::marker::PhantomData;

/// A branded type for type-level distinction.
///
/// This is similar to TypeScript's branded types, using Rust's newtype pattern
/// with a phantom type parameter for compile-time type safety.
///
/// # Examples
///
/// ```
/// use json_joy_util::types::Branded;
///
/// // Create a branded string type for user IDs
/// struct UserIdTag;
/// type UserId = Branded<String, UserIdTag>;
///
/// fn get_user(id: UserId) -> String {
///     id.into_inner()
/// }
///
/// let user_id = UserId::from_inner("user-123".to_string());
/// assert_eq!(get_user(user_id), "user-123");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Branded<T, B> {
    inner: T,
    _marker: PhantomData<B>,
}

impl<T, B> Branded<T, B> {
    /// Create a new branded value from the inner type.
    pub fn from_inner(inner: T) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }

    /// Extract the inner value from the branded type.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Get a reference to the inner value.
    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T, B> std::ops::Deref for Branded<T, B> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, B> AsRef<T> for Branded<T, B> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

/// A type that may be a single value or an array of values.
///
/// This is similar to TypeScript's `MaybeArray<T>` type.
/// In Rust, use `Either<T, Vec<T>>` from the `either` crate for this pattern.
pub type MaybeArray<T> = Vec<T>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branded_type() {
        struct UserIdTag;
        type UserId = Branded<String, UserIdTag>;

        let id = UserId::from_inner("user-123".to_string());
        assert_eq!(id.inner(), &"user-123".to_string());
        assert_eq!(id.into_inner(), "user-123".to_string());
    }

    #[test]
    fn test_branded_type_deref() {
        struct EmailTag;
        type Email = Branded<String, EmailTag>;

        let email = Email::from_inner("test@example.com".to_string());
        // Can use string methods via Deref
        assert!(email.contains('@'));
    }
}
