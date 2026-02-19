use std::fmt;
use std::sync::Arc;

/// Efficient slice reference to a portion of byte array data without copying.
///
/// Mirrors upstream `radix/Slice.ts`.
#[derive(Clone, Eq, PartialEq)]
pub struct Slice {
    pub data: Arc<[u8]>,
    pub start: usize,
    pub length: usize,
}

impl Slice {
    pub fn new(data: Arc<[u8]>, start: usize, length: usize) -> Self {
        Self {
            data,
            start,
            length,
        }
    }

    /// Get byte at the given index within this slice.
    pub fn at(&self, index: usize) -> u8 {
        if index >= self.length {
            panic!(
                "Index {index} out of bounds for slice of length {}",
                self.length
            );
        }
        self.data[self.start + index]
    }

    /// Create a new slice that represents a substring of this slice.
    pub fn substring(&self, start: usize, length: Option<usize>) -> Self {
        if start > self.length {
            panic!(
                "Start {start} out of bounds for slice of length {}",
                self.length
            );
        }

        let new_length = match length {
            Some(len) => len.min(self.length - start),
            None => self.length - start,
        };

        Self {
            data: Arc::clone(&self.data),
            start: self.start + start,
            length: new_length,
        }
    }

    /// Compare this slice with another slice for equality.
    pub fn equals(&self, other: &Self) -> bool {
        if self.length != other.length {
            return false;
        }
        for i in 0..self.length {
            if self.at(i) != other.at(i) {
                return false;
            }
        }
        true
    }

    /// Compare this slice with another slice lexicographically.
    pub fn compare(&self, other: &Self) -> i32 {
        let min_length = self.length.min(other.length);
        for i in 0..min_length {
            let this_byte = self.at(i);
            let other_byte = other.at(i);
            if this_byte != other_byte {
                return this_byte as i32 - other_byte as i32;
            }
        }
        self.length as i32 - other.length as i32
    }

    /// Create a new byte vector containing the data from this slice.
    pub fn to_uint8_array(&self) -> Vec<u8> {
        self.data[self.start..self.start + self.length].to_vec()
    }

    /// Find the length of common prefix between this slice and another slice.
    pub fn get_common_prefix_length(&self, other: &Self) -> usize {
        let len = self.length.min(other.length);
        let mut i = 0;
        while i < len && self.at(i) == other.at(i) {
            i += 1;
        }
        i
    }

    /// Create a slice from a byte vector.
    pub fn from_uint8_array(data: Vec<u8>) -> Self {
        let arc: Arc<[u8]> = Arc::from(data);
        let len = arc.len();
        Self {
            data: arc,
            start: 0,
            length: len,
        }
    }
}

impl fmt::Debug for Slice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for Slice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let body = self
            .to_uint8_array()
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "Slice({body})")
    }
}
