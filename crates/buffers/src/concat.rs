//! Byte slice concatenation utilities.

/// Concatenates two byte slices into a new vector.
///
/// # Example
///
/// ```
/// use json_joy_buffers::concat;
///
/// let result = concat(&[1, 2], &[3, 4]);
/// assert_eq!(result, vec![1, 2, 3, 4]);
/// ```
pub fn concat(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut res = Vec::with_capacity(a.len() + b.len());
    res.extend_from_slice(a);
    res.extend_from_slice(b);
    res
}

/// Concatenates a list of byte slices into a new vector.
///
/// # Example
///
/// ```
/// use json_joy_buffers::concat_list;
///
/// let result = concat_list(&[&[1, 2][..], &[3, 4][..], &[5][..]]);
/// assert_eq!(result, vec![1, 2, 3, 4, 5]);
/// ```
pub fn concat_list(list: &[&[u8]]) -> Vec<u8> {
    let total_size: usize = list.iter().map(|s| s.len()).sum();
    let mut res = Vec::with_capacity(total_size);
    for item in list {
        res.extend_from_slice(item);
    }
    res
}

/// Converts a list of byte slices to a single vector.
///
/// Returns an empty vec if the list is empty, returns the single slice
/// if there's only one, otherwise concatenates all slices.
///
/// # Example
///
/// ```
/// use json_joy_buffers::list_to_uint8;
///
/// assert_eq!(list_to_uint8(&[]), vec![]);
/// assert_eq!(list_to_uint8(&[&[1, 2][..]]), vec![1, 2]);
/// assert_eq!(list_to_uint8(&[&[1][..], &[2][..]]), vec![1, 2]);
/// ```
pub fn list_to_uint8(list: &[&[u8]]) -> Vec<u8> {
    match list.len() {
        0 => Vec::new(),
        1 => list[0].to_vec(),
        _ => concat_list(list),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concat() {
        assert_eq!(concat(&[1, 2], &[3, 4]), vec![1, 2, 3, 4]);
        assert_eq!(concat(&[], &[1, 2]), vec![1, 2]);
        assert_eq!(concat(&[1, 2], &[]), vec![1, 2]);
        assert_eq!(concat(&[] as &[u8], &[] as &[u8]), vec![]);
    }

    #[test]
    fn test_concat_list() {
        assert_eq!(concat_list(&[]), vec![]);
        assert_eq!(concat_list(&[&[1][..]]), vec![1]);
        assert_eq!(concat_list(&[&[1, 2][..], &[3, 4][..]]), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_list_to_uint8() {
        assert_eq!(list_to_uint8(&[]), vec![]);
        assert_eq!(list_to_uint8(&[&[1, 2][..]]), vec![1, 2]);
        assert_eq!(
            list_to_uint8(&[&[1][..], &[2][..], &[3][..]]),
            vec![1, 2, 3]
        );
    }
}
