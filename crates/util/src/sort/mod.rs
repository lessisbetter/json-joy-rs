//! Sorting utilities.
//!
//! Provides insertion sort implementations optimized for small arrays.

mod insertion;

pub use insertion::{insertion_sort, insertion_sort_by, insertion_sort_by_key};
