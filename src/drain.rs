use std::iter::FusedIterator;
use std::ops::Range;
use std::ptr;
use super::AltDeque;

/// A draining iterator over the elements of an `AltDeque`.
///
/// This `struct` is created by the [`drain`] method on [`AltDeque`]. See it's
/// documentation for more information.
///
/// [`drain`]: AltDeque::drain
#[derive(Debug)]
pub struct Drain<'a, T> {
    inner: &'a mut AltDeque<T>,
    old_head: usize,
    old_tail: usize,
    // the original draining range, this is not modified
    range: Range<usize>,
    // the element that `.next()` returns
    start: usize,
    // the element after the one `.next_back()` return
    end: usize,
}

impl<'a, T> Drain<'a, T> {
    pub(super) fn new(deque: &'a mut AltDeque<T>, old_head: usize, old_tail: usize, range: Range<usize>) -> Self {
        let Range { start, end } = range;
        Self { inner: deque, old_head, old_tail, range, start, end }
    }
}

impl<T> Iterator for Drain<'_, T> {
    type Item = T;
    
    fn next(&mut self) -> Option<T> {
        if self.start < self.end {
            let front_len = self.inner.cap() - self.old_tail;
            let start = self.start;
            self.start += 1;
            if start < front_len {
                unsafe { Some(ptr::read(self.inner.buf_add(start + self.old_tail))) }
            } else {
                unsafe { Some(ptr::read(self.inner.buf_add(start - front_len))) }
            }
        } else {
            None
        }
    }
    
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.end - self.start;
        (len, Some(len))
    }
}

impl<T> DoubleEndedIterator for Drain<'_, T> {
    fn next_back(&mut self) -> Option<T> {
        if self.start < self.end {
            let front_len = self.inner.cap() - self.old_tail;
            self.end -= 1;
            if self.end < front_len {
                unsafe { Some(ptr::read(self.inner.buf_add(self.end + self.old_tail))) }
            } else {
                unsafe { Some(ptr::read(self.inner.buf_add(self.end - front_len))) }
            }
        } else {
            None
        }
    }
}

impl<T> Drop for Drain<'_, T> {
    fn drop(&mut self) {
        while let Some(item) = self.next() {
            drop(item);
        }
        
        let front_len = self.inner.cap() - self.old_tail;
        if self.range.start < front_len {
            if self.range.end <= front_len {
                let new_tail = self.inner.cap() - self.range.len();
                unsafe {
                    self.inner.copy(self.old_tail, new_tail, self.range.start);
                }
                self.inner.tail = new_tail;
            } else {
                let new_head = self.old_head - (self.range.end - front_len);
                let new_tail = self.inner.cap() - self.range.start;
                unsafe {
                    self.inner.copy(self.old_tail, new_tail, self.range.start);
                    self.inner.copy(self.range.end - front_len, 0, new_head);
                }
                self.inner.head = new_head;
                self.inner.tail = new_tail;
            }
        } else {
            unsafe {
                let end = self.range.end - front_len;
                let start = self.range.start - front_len;
                self.inner.copy(end, start, self.old_head - end);
            }
            self.inner.head = self.old_head - self.range.len();
        }
    }
}

impl<T> ExactSizeIterator for Drain<'_, T> {}

impl<T> FusedIterator for Drain<'_, T> {}
