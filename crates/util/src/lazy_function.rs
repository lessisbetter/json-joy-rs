use std::sync::{Arc, Mutex, OnceLock};

/// A lazily initialized function wrapper.
///
/// This is the Rust equivalent of the TypeScript `lazy` function that wraps
/// a factory function and only calls it on first invocation.
///
/// # Thread Safety
///
/// The initialization is thread-safe and will only occur once, even if
/// multiple threads call the function simultaneously.
///
/// # Examples
///
/// ```
/// use std::sync::atomic::{AtomicUsize, Ordering};
/// use std::sync::Arc;
/// use json_joy_util::lazy_function::LazyFn;
///
/// let call_count = Arc::new(AtomicUsize::new(0));
/// let count_clone = call_count.clone();
///
/// let lazy_fn = LazyFn::new(move || {
///     count_clone.fetch_add(1, Ordering::SeqCst);
///     |x: i32| x * 2
/// });
///
/// // Factory hasn't been called yet
/// assert_eq!(call_count.load(Ordering::SeqCst), 0);
///
/// // First access initializes the function
/// let func = lazy_fn.get();
/// assert_eq!(func(5), 10);
/// assert_eq!(call_count.load(Ordering::SeqCst), 1);
///
/// // Subsequent accesses return cached function
/// let func2 = lazy_fn.get();
/// assert_eq!(func2(7), 14);
/// assert_eq!(call_count.load(Ordering::SeqCst), 1);
/// ```
pub struct LazyFn<F, R>
where
    F: FnOnce() -> R,
    R: Clone,
{
    factory: OnceLock<R>,
    factory_fn: Arc<Mutex<Option<F>>>,
}

impl<F, R> LazyFn<F, R>
where
    F: FnOnce() -> R,
    R: Clone,
{
    /// Create a new lazy function wrapper.
    pub fn new(factory: F) -> Self {
        Self {
            factory: OnceLock::new(),
            factory_fn: Arc::new(Mutex::new(Some(factory))),
        }
    }

    /// Get the cached value, initializing it if necessary.
    pub fn get(&self) -> &R {
        self.factory.get_or_init(|| {
            let factory = self.factory_fn.lock().unwrap().take().unwrap();
            factory()
        })
    }
}

/// A simpler lazy function wrapper for closures that take no arguments.
///
/// # Examples
///
/// ```
/// use json_joy_util::lazy_function::lazy;
///
/// let expensive_value = lazy(|| {
///     // Expensive computation
///     42
/// });
///
/// // Value is computed on first access
/// assert_eq!(*expensive_value.get(), 42);
/// ```
pub fn lazy<T, F>(f: F) -> Lazy<T, F>
where
    F: FnOnce() -> T,
{
    Lazy::new(f)
}

/// A lazily initialized value.
pub struct Lazy<T, F = fn() -> T>
where
    F: FnOnce() -> T,
{
    value: OnceLock<T>,
    init: Mutex<Option<F>>,
}

impl<T, F> Lazy<T, F>
where
    F: FnOnce() -> T,
{
    /// Create a new lazy value.
    pub fn new(f: F) -> Self {
        Self {
            value: OnceLock::new(),
            init: Mutex::new(Some(f)),
        }
    }

    /// Get the value, initializing it if necessary.
    pub fn get(&self) -> &T {
        self.value.get_or_init(|| {
            let init = self.init.lock().unwrap().take().unwrap();
            init()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_lazy_value() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let lazy_val = lazy(move || {
            count_clone.fetch_add(1, Ordering::SeqCst);
            42
        });

        // Factory hasn't been called yet
        assert_eq!(call_count.load(Ordering::SeqCst), 0);

        // First access initializes the value
        assert_eq!(*lazy_val.get(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Subsequent accesses return cached value
        assert_eq!(*lazy_val.get(), 42);
        assert_eq!(*lazy_val.get(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_lazy_fn() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let lazy_fn = LazyFn::new(move || {
            count_clone.fetch_add(1, Ordering::SeqCst);
            |x: i32| x * 2
        });

        // Factory hasn't been called yet
        assert_eq!(call_count.load(Ordering::SeqCst), 0);

        // First access initializes the function
        let func = lazy_fn.get();
        assert_eq!(func(5), 10);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Subsequent accesses return cached function
        let func2 = lazy_fn.get();
        assert_eq!(func2(7), 14);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
