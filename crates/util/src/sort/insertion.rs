use std::cmp::Ordering;

/// Insertion sort for slices with natural ordering.
///
/// This is generally faster than the built-in sort for small slices (typically < 32 elements).
/// For larger slices, the standard library's `sort` method is recommended as it uses
/// a more sophisticated algorithm (pattern-defeating quicksort).
///
/// # Performance
///
/// - Time complexity: O(n²) worst case, O(n) best case (already sorted)
/// - Space complexity: O(1) - sorts in place
/// - Best for: Small arrays, nearly-sorted data, or when simplicity is preferred
///
/// # Examples
///
/// ```
/// use json_joy_util::sort::insertion_sort;
///
/// let mut arr = vec![3, 1, 4, 1, 5, 9, 2, 6];
/// insertion_sort(&mut arr);
/// assert_eq!(arr, vec![1, 1, 2, 3, 4, 5, 6, 9]);
/// ```
pub fn insertion_sort<T: Ord>(arr: &mut [T]) {
    let len = arr.len();
    for i in 1..len {
        let mut j = i;
        while j > 0 && arr[j - 1] > arr[j] {
            arr.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Insertion sort with a custom comparator.
///
/// # Examples
///
/// ```
/// use json_joy_util::sort::insertion_sort_by;
/// use std::cmp::Ordering;
///
/// let mut arr = vec![3, 1, 4, 1, 5];
/// insertion_sort_by(&mut arr, |a, b| b.cmp(a)); // Descending order
/// assert_eq!(arr, vec![5, 4, 3, 1, 1]);
/// ```
pub fn insertion_sort_by<T, F>(arr: &mut [T], mut compare: F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    let len = arr.len();
    for i in 1..len {
        let mut j = i;
        while j > 0 && compare(&arr[j - 1], &arr[j]) == Ordering::Greater {
            arr.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Insertion sort with a key extraction function.
///
/// # Examples
///
/// ```
/// use json_joy_util::sort::insertion_sort_by_key;
///
/// let mut arr = vec!["banana", "apple", "cherry"];
/// insertion_sort_by_key(&mut arr, |s| s.len());
/// assert_eq!(arr, vec!["apple", "banana", "cherry"]);
/// ```
pub fn insertion_sort_by_key<T, K, F>(arr: &mut [T], mut key: F)
where
    K: Ord,
    F: FnMut(&T) -> K,
{
    let len = arr.len();
    for i in 1..len {
        let mut j = i;
        while j > 0 && key(&arr[j - 1]) > key(&arr[j]) {
            arr.swap(j - 1, j);
            j -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insertion_sort_empty() {
        let mut arr: Vec<i32> = vec![];
        insertion_sort(&mut arr);
        let expected: Vec<i32> = vec![];
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_insertion_sort_single() {
        let mut arr = vec![1];
        insertion_sort(&mut arr);
        assert_eq!(arr, vec![1]);
    }

    #[test]
    fn test_insertion_sort_sorted() {
        let mut arr = vec![1, 2, 3, 4, 5];
        insertion_sort(&mut arr);
        assert_eq!(arr, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_insertion_sort_reverse() {
        let mut arr = vec![5, 4, 3, 2, 1];
        insertion_sort(&mut arr);
        assert_eq!(arr, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_insertion_sort_random() {
        let mut arr = vec![3, 1, 4, 1, 5, 9, 2, 6];
        insertion_sort(&mut arr);
        assert_eq!(arr, vec![1, 1, 2, 3, 4, 5, 6, 9]);
    }

    #[test]
    fn test_insertion_sort_strings() {
        let mut arr = vec!["banana", "apple", "cherry"];
        insertion_sort(&mut arr);
        assert_eq!(arr, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn test_insertion_sort_by_descending() {
        let mut arr = vec![3, 1, 4, 1, 5];
        insertion_sort_by(&mut arr, |a, b| b.cmp(a));
        assert_eq!(arr, vec![5, 4, 3, 1, 1]);
    }

    #[test]
    fn test_insertion_sort_by_key_length() {
        let mut arr = vec!["aaa", "b", "cc"];
        insertion_sort_by_key(&mut arr, |s| s.len());
        assert_eq!(arr, vec!["b", "cc", "aaa"]);
    }
}
