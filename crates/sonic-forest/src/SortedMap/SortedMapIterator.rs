use super::constants::IteratorType;
use super::util::throw_iterator_access_error;

#[derive(Clone, Debug)]
pub struct OrderedMapIterator {
    pos: usize,
    len: usize,
    pub iterator_type: IteratorType,
}

impl OrderedMapIterator {
    pub fn new(pos: usize, len: usize, iterator_type: IteratorType) -> Self {
        Self {
            pos,
            len,
            iterator_type,
        }
    }

    pub fn pre(&mut self) -> &mut Self {
        match self.iterator_type {
            IteratorType::Normal => {
                if self.len == 0 || self.pos == 0 {
                    throw_iterator_access_error();
                }
                self.pos -= 1;
            }
            IteratorType::Reverse => {
                if self.len == 0 {
                    throw_iterator_access_error();
                }
                if self.pos == self.len - 1 {
                    throw_iterator_access_error();
                }
                if self.pos == self.len {
                    self.pos = 0;
                } else {
                    self.pos += 1;
                }
            }
        }
        self
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> &mut Self {
        match self.iterator_type {
            IteratorType::Normal => {
                if self.pos == self.len {
                    throw_iterator_access_error();
                }
                self.pos += 1;
            }
            IteratorType::Reverse => {
                if self.pos == self.len {
                    throw_iterator_access_error();
                }
                if self.pos == 0 {
                    self.pos = self.len;
                } else {
                    self.pos -= 1;
                }
            }
        }
        self
    }

    pub fn index(&self) -> usize {
        if self.pos == self.len {
            self.len.saturating_sub(1)
        } else {
            self.pos
        }
    }

    pub fn is_accessible(&self) -> bool {
        self.pos != self.len
    }

    pub fn copy(&self) -> Self {
        self.clone()
    }

    pub fn equals(&self, other: &Self) -> bool {
        self.pos == other.pos && self.iterator_type == other.iterator_type
    }

    pub(crate) fn position(&self) -> Option<usize> {
        if self.is_accessible() {
            Some(self.pos)
        } else {
            None
        }
    }

    pub(crate) fn sync_len(&mut self, len: usize) {
        self.len = len;
        if self.pos > len {
            self.pos = len;
        }
    }

    pub(crate) fn set_position(&mut self, pos: usize) {
        self.pos = pos;
    }
}
