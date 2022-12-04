use std::iter::FusedIterator;

use super::AltDeque;

/// An owning iterator over the elements of an `AltDeque`.
///
/// This `struct` is created by the [`into_iter`] method on [`AltDeque`] (provided by the
/// [`IntoIterator`] trait). See it's documentation for more information.
///
/// [`into_iter`]: AltDeque::into_iter
/// [`IntoIterator`]: core::iter::IntoIterator
#[derive(Debug, Clone)]
pub struct IntoIter<T> {
    inner: AltDeque<T>,
}

impl<T> IntoIter<T> {
    pub(super) fn new(inner: AltDeque<T>) -> Self {
        IntoIter { inner }
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.inner.pop_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.inner.len();
        (len, Some(len))
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        self.inner.pop_back()
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {}

impl<T> FusedIterator for IntoIter<T> {}
