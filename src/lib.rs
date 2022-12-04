//! An Alernative Deque implemention written in Rust.
//!
//! AltDeque is an alternative to the standard library's `VecDeque`. It exposes
//! mostly the same methods and has the performance characteristics. But
//! instead of using a ring buffer to achieve efficient insertion on both
//! ends, it uses two stacks. One stack to `push` and `pop` elements for each
//! end of the deuque. If `pop` is called on one end but it's stack is empty, then all
//! elements from the other stack are removed, reversed and put into it. This
//! operation takes *O(n)* time (where n is the length of the deque) but
//! after it *n* elements can be popped in constant time resulting in an
//! amortized runtime of *O(1)* for popping.
//!
//! For more efficient memory usage both stacks are located at the ends of one
//! allocated buffer:
//! ```
//!         growth ->               <- growth
//! +- back stack --+               +- front stack -+
//! |               |               |               |
//! v               v               v               v
//! +---+---+---+---+---+---+---+---+---+---+---+---+
//! | 4 | 5 | 6 | 7 |   |   |   |   | 0 | 1 | 2 | 3 |
//! +---+---+---+---+---+---+---+---+---+---+---+---+
//!                   |               |
//!             head -+               +- tail
//! ```
//!
//! This stack based approach has some advantages over a ringbuffer:
//! - no need for masks or modular arithmetic to access elements
//! - no need for a power of 2 capacity
//! - no need to always leave one element empty
//!
//! But it also has a few disadvantges:
//! - accessing elemnts needs an additional branch to check in which stack they are
//! - popping elements is only *amortized* constant time, a single pop-call will
//!   take linear time if the coresponding stack is empty
//! - popping elements alternating from both sides is very inefficient as all
//!   elements need to be moved from one side to the other every time the side is changed
//!
//! In my simple tests `AltDeque` and `VecDeque` are about equally fast for a simply
//! `push_back` and `pop_front` workload.
//!
//!
//! Some of the code and a lot of the docs and examples are taken from the code in the
//! [rust repository](https://github.com/rust-lang/rust/), so credits to it's contributors.

use core::cmp::{self, Ordering};
use core::hash::{Hash, Hasher};
use core::ops::{Bound, Index, IndexMut, Range, RangeBounds};

use std::fmt;
use std::iter::{repeat_with, Chain};
use std::mem::{self, ManuallyDrop};
use std::ptr;
use std::slice;

#[macro_use]
mod macros;

mod drain;
mod into_iter;
mod raw_vec;

pub use drain::Drain;
pub use into_iter::IntoIter;
use raw_vec::RawVec;

#[cfg(test)]
mod tests;

pub type Iter<'a, T> = Chain<slice::Iter<'a, T>, slice::Iter<'a, T>>;

pub type IterMut<'a, T> = Chain<slice::IterMut<'a, T>, slice::IterMut<'a, T>>;

/// Runs the destructor for all items in the slice when it gets dropped (normally or during unwinding).
/// Used by AltDeque::drop and some other methods to ensure that elements in the back stack are dropped
/// even when the destructed of an element in the front stack panics.
struct Dropper<'a, T>(&'a mut [T]);

impl<'a, T> Drop for Dropper<'a, T> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.0);
        }
    }
}

/// An alternative deque implementation to [`VecDeque`] in the standard library.
///
/// See the [module-level documentation](./index.html) for more details.
///
/// [`VecDeque`]: std::collections::VecDeque
pub struct AltDeque<T> {
    // Tail and head are pointers into the buffer.
    // Tail always points to the first element that could be read,
    // Head always points to where data should be written.
    // If tail == head the buffer is full.
    // If head == 0 and tail == capacity the buffer is empty.
    // The length of the buffer is defined as head + capacity - tail.
    // 0 <= head <= tail <= capacity <= usize::MAX
    tail: usize,
    head: usize,
    buf: RawVec<T>,
}

impl<T> AltDeque<T> {
    /// Creates an empty deque.
    ///
    /// Examples
    ///
    /// ```
    /// use altdeque::AltDeque;
    ///
    /// let deque: AltDeque<i32> = AltDeque::new();
    ///```
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates an empty deque with space for at least `capacity` elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use altdeque::AltDeque;
    ///
    /// let deque: AltDeque<i32> = AltDeque::with_capacity(10);
    /// assert!(deque.capacity() >= 10)
    ///```
    pub fn with_capacity(capacity: usize) -> Self {
        let buf = RawVec::with_capacity(capacity);
        Self { tail: buf.capacity(), head: 0, buf }
    }

    /// Returns the number of elements the deque can hold without reallocating.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let deque: AltDeque<i32> = AltDeque::with_capacity(10);
    /// assert!(deque.capacity() >= 10);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.cap()
    }

    /// Returns the number of elements in the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let deque: AltDeque<i32> = AltDeque::from([1, 2, 3]);
    /// assert_eq!(deque.len(), 3);
    /// ```
    pub fn len(&self) -> usize {
        // this cannot overflow because head <= tail <= cap
        self.cap() - self.tail + self.head
    }

    /// Returns wether the deque is empty or not.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    /// assert!(deque.is_empty());
    /// deque.push_back(42);
    /// assert!(!deque.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.head == 0 && self.tail == self.cap()
    }

    /// Returns a pair of slices which contain, in order, the contents of the deque. These are
    /// equal the front stack and the back stack used internally.
    ///
    /// If [`make_contiguous`] was previously called, all elements of the deque will be in the
    /// first slice and the second slice will be empty.
    ///
    /// [`make_contiguous`]: AltDeque::make_contiguous
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    ///
    /// deque.push_back(0);
    /// deque.push_back(1);
    /// deque.push_back(2);
    ///
    /// assert_eq!(deque.as_slices(), (&[][..], &[0, 1, 2][..]));
    ///
    /// deque.push_front(3);
    /// deque.push_front(4);
    ///
    /// assert_eq!(deque.as_slices(), (&[4, 3][..], &[0, 1, 2][..]));
    /// ```
    pub fn as_slices(&self) -> (&[T], &[T]) {
        // SAFETY: all elements in the ranges [ptr, head) and [tail, cap) are valid
        unsafe {
            let front = slice::from_raw_parts(self.buf_add(self.tail), self.cap() - self.tail);
            let back = slice::from_raw_parts(self.buf.ptr(), self.head);
            (front, back)
        }
    }

    /// Returns a mutable pair of slices which contain, in order, the contents of the deque.
    ///
    /// See the non-mutable version [`as_slices`] for details and examples.
    ///
    /// [`as_slices`]: AltDeque::as_slices
    pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        // SAFETY: all elements in the ranges [ptr, head) and [tail, cap) are valid and do not overlap
        unsafe {
            let front = slice::from_raw_parts_mut(self.buf_add(self.tail), self.cap() - self.tail);
            let back = slice::from_raw_parts_mut(self.buf.ptr(), self.head);
            (front, back)
        }
    }

    /// Provides a reference to the element at the given index.
    ///
    /// Element at index 0 is the front of the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let deque = AltDeque::from([1, 2, 3]);
    /// assert_eq!(deque.get(1), Some(&2));
    /// ```
    pub fn get(&self, index: usize) -> Option<&T> {
        let front_len = self.cap() - self.tail;
        if index < front_len + self.head {
            if index < front_len {
                // SAFETY: index < cap - tail -> tail <= tail + index < cap
                unsafe { Some(&*self.buf_add(self.tail + index)) }
            } else {
                // SAFETY: index >= cap - tail && index < len -> 0 <= index - front_len < head
                unsafe { Some(&*self.buf_add(index - front_len)) }
            }
        } else {
            None
        }
    }

    /// Provides a mutable reference to the element at the given index.
    ///
    /// Element at index 0 is the front of the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3]);
    /// *deque.get_mut(1).unwrap() += 40;
    /// assert_eq!(deque.get(1), Some(&42));
    /// ```
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let front_len = self.cap() - self.tail;
        if index < front_len + self.head {
            if index < front_len {
                // SAFETY: index < cap - tail -> tail <= tail + index < cap
                unsafe { Some(&mut *self.buf_add(self.tail + index)) }
            } else {
                // SAFETY: index >= cap - tail && index < len -> 0 <= index - front_len < head
                unsafe { Some(&mut *self.buf_add(index - front_len)) }
            }
        } else {
            None
        }
    }

    /// Reserves the minimum capacity for at least `additional` more elements to be inserted in the
    /// given deque. Does nothing if the capacity is already sufficient.
    ///
    /// Note that the allocator may give the collection more space than it requests. Therefore
    /// capacity can not be relied upon to be precisely minimal. Prefer [`reserve`] if future
    /// insertions are expected.
    ///
    /// [`reserve`]: AltDeque::reserve
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4]);
    /// deque.reserve_exact(10);
    /// assert!(deque.capacity() >= 14);
    /// ```
    pub fn reserve_exact(&mut self, additional: usize) {
        let old_cap = self.cap();
        let used_cap = self.len();
        // this call will panic on overflow or if T is zero-sized
        // and do nothing if capacity is already sufficient
        self.buf.reserve_exact(used_cap, additional);
        // SAFETY: old_cap is correct
        unsafe {
            self.handle_capacity_increase(old_cap);
        }
    }

    /// Reserves capacity for at least `additional` more elements to be inserted in the given
    /// deque. The collection may reserve more space to speculatively avoid frequent reallocations.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4]);
    /// deque.reserve(10);
    /// assert!(deque.capacity() >= 14);
    /// ```
    pub fn reserve(&mut self, additional: usize) {
        let old_cap = self.cap();
        let used_cap = self.len();
        // this call will panic on overflow or if T is zero-sized
        // and do nothing if capacity is already sufficient
        self.buf.reserve(used_cap, additional);
        // SAFETY: old_cap is correct
        unsafe {
            self.handle_capacity_increase(old_cap);
        }
    }

    /// Modifies the deque in-place so that `len()` is equal to `new_len`, either by removing
    /// excess elements from the back or by appending elements generated by calling `generator` to
    /// the back.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2]);
    /// let mut i = 3;
    ///
    /// deque.resize_with(5, || { i += 1; i });
    /// assert_eq!(deque, [1, 2, 4, 5, 6]);
    ///
    /// deque.resize_with(3, || unreachable!());
    /// assert_eq!(deque, [1, 2, 4]);
    /// ```
    pub fn resize_with<F>(&mut self, new_len: usize, generator: F)
    where
        F: FnMut() -> T,
    {
        let len = self.len();
        if new_len > len {
            self.extend(repeat_with(generator).take(new_len - len));
        } else {
            self.truncate(new_len);
        }
    }

    /// Shrinks the capacity of the deque as much as possible.
    ///
    /// It will drop down as close as possible to the length but the allocator may still inform the
    /// deque that there is space for a few more elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::with_capacity(16);
    /// deque.extend(0..4);
    /// assert_eq!(deque.capacity(), 16);
    /// deque.shrink_to_fit();
    /// assert!(deque.capacity() >= 4);
    /// ```
    pub fn shrink_to_fit(&mut self) {
        self.shrink_to(0);
    }

    /// Shrinks the capacity of the deque with a lower bound.
    ///
    /// The capacity will remain at least as large as both the length and the supplied lower bound.
    ///
    /// If the current capacity is less than the lower bound, this is a no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::with_capacity(16);
    /// deque.extend(0..4);
    /// assert_eq!(deque.capacity(), 16);
    /// deque.shrink_to(7);
    /// assert!(deque.capacity() >= 7);
    /// deque.shrink_to(0);
    /// assert!(deque.capacity() >= 4);
    /// ```
    pub fn shrink_to(&mut self, min_capacity: usize) {
        if min_capacity >= self.capacity() {
            return;
        }

        let target_cap = cmp::max(min_capacity, self.len());
        let front_len = self.cap() - self.tail;
        let new_tail = target_cap - front_len;

        // SAFETY: target_cap >= len >= front_len -> we can move front_len elements from tail to new_tail
        unsafe {
            self.copy(self.tail, new_tail, front_len);
        }
        self.tail = new_tail;
        self.buf.shrink_to_fit(target_cap);

        if self.cap() > target_cap {
            // oh no, more capacity remained than we requested
            let new_tail = self.cap() - front_len;
            // SAFETY: cap > target_cap -> we can move front_len elements from tail to new_tail
            unsafe {
                self.copy(self.tail, new_tail, front_len);
            }
            self.tail = new_tail;
        }
    }

    /// Shortens the deque, keeping the first `len` elements and dropping the rest.
    ///
    /// If `len` is greater than the deque's current length, this is a no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4]);
    /// deque.truncate(2);
    /// assert_eq!(deque, [1, 2]);
    /// ```
    pub fn truncate(&mut self, len: usize) {
        /// Runs the final step of trunacte (moving elements around) even if the destructor of a
        /// dropped element panics.
        struct DropGuard<T>{ ptr: *mut AltDeque<T>, old_tail: usize, len: usize }

        impl<T> Drop for DropGuard<T> {
            fn drop(&mut self) {
                // SAFETY: we got ptr from a mutable reference
                let deque = unsafe { self.ptr.as_mut().unwrap_unchecked() };
                deque.tail = deque.cap() - self.len;
                // SAFETY: len <= old front len -> we can copy len elements from old_tail to cap - len
                unsafe {
                    deque.copy(self.old_tail, deque.tail, self.len);
                }
            }
        }

        if len > self.len() {
            return;
        }

        // SAFETY::
        // * Any slice passed to `drop_in_place` is valid; the second case has `len <= front.len()`
        //   and returning on `len > self.len()` ensures `begin <= back.len()` in the first case.
        // * The tail of the AltDeque is moved before calling `drop_in_place`, so no value is
        //   dropped twice if `drop_in_place` panics.
        unsafe {
            let (front, back) = self.as_mut_slices();
            if len > front.len() {
                let begin = len - front.len();
                let drop_back = back.get_unchecked_mut(begin..) as *mut _;
                self.head = begin;
                ptr::drop_in_place(drop_back);
            } else {
                let drop_back = back as *mut _;
                let drop_front = front.get_unchecked_mut(len..) as *mut _;

                // Make sure the remaining elements in front are moved to the freed space even if a destructor panics.
                let _guard = DropGuard { ptr: self as *mut _, old_tail: self.tail, len};
                self.head = 0;
                // temp set tail to cap so that no dropped elements can be accessed even if something wents horribly wrong
                self.tail = self.cap();
                {
                    // Make sure the second half is dropped even when a destructor in the first one panics.
                    let _back_dropper = Dropper(&mut *drop_back);
                    ptr::drop_in_place(drop_front);
                }
            }
        }
    }

    /// Clears the deque, removing all elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    /// deque.push_back(1);
    /// deque.clear();
    /// assert!(deque.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Returns `true` if the deque contains an element equal to the given value.
    ///
    /// This operation is *O(n)*.
    ///
    /// Note that if you have a sorted `AltDeque`, [`binary_search`] may be faster.
    ///
    /// [`binary_search`]: AltDeque::binary_search
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    ///
    /// deque.push_back(0);
    /// deque.push_back(1);
    ///
    /// assert!(deque.contains(&1));
    /// assert!(!deque.contains(&4));
    /// ```
    pub fn contains(&self, x: &T) -> bool
    where
        T: PartialEq<T>,
    {
        let (a, b) = self.as_slices();
        a.contains(x) || b.contains(x)
    }

    /// Provides a reference to the front element, or `None` if the deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    /// assert_eq!(deque.front(), None);
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// assert_eq!(deque.front(), Some(&1));
    /// ```
    pub fn front(&self) -> Option<&T> {
        if self.tail != self.cap() {
            // SAFETY: tail != cap -> tail points at a valid element
            unsafe { Some(&*self.buf_add(self.tail)) }
        } else if self.head != 0 {
            // SAFETY: head != 0 -> the element at ptr + 0 is valid
            unsafe { Some(&*self.buf_add(0)) }
        } else {
            None
        }
    }

    /// Provides a mutable reference to the front element, or `None` if the deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    /// assert_eq!(deque.front_mut(), None);
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// *deque.front_mut().unwrap() += 10;
    /// assert_eq!(deque.front_mut(), Some(&mut 11));
    pub fn front_mut(&mut self) -> Option<&mut T> {
        if self.tail != self.cap() {
            // SAFETY: tail != cap -> tail points at a valid element
            unsafe { Some(&mut *self.buf_add(self.tail)) }
        } else if self.head != 0 {
            // SAFETY: head != 0 -> the element at ptr + 0 is valid
            unsafe { Some(&mut *self.buf_add(0)) }
        } else {
            None
        }
    }

    /// Provides a reference to the back element, or `None` if the deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    /// assert_eq!(deque.back(), None);
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// assert_eq!(deque.back(), Some(&2));
    pub fn back(&self) -> Option<&T> {
        if self.head != 0 {
            // SAFETY: head != 0 -> the element at head - 1 is valid
            unsafe { Some(&*self.buf_add(self.head - 1)) }
        } else if self.tail != self.cap() {
            // SAFETY: tail != cap -> the element at cap - 1 is valid
            unsafe { Some(&*self.buf_add(self.cap() - 1)) }
        } else {
            None
        }
    }

    /// Provides a mutable reference to the back element, or `None` if the deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    /// assert_eq!(deque.back_mut(), None);
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// *deque.back_mut().unwrap() += 10;
    /// assert_eq!(deque.back_mut(), Some(&mut 12));
    pub fn back_mut(&mut self) -> Option<&mut T> {
        if self.head != 0 {
            // SAFETY: head != 0 -> the element at head - 1 is valid
            unsafe { Some(&mut *self.buf_add(self.head - 1)) }
        } else if self.tail != self.cap() {
            // SAFETY: tail != cap -> the element at cap - 1 is valid
            unsafe { Some(&mut *self.buf_add(self.cap() - 1)) }
        } else {
            None
        }
    }

    /// Removes the first element and returns it, or `None` if the deque is empty.
    ///
    /// Be careful when also using [`pop_back`]. Popping elements from both sides can be very
    /// inefficient. Use [`VecDeque`] if in doubt.
    ///
    /// [`pop_back`]: AltDeque::pop_back
    /// [`VecDeque`]: std::collections::VecDeque
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2]);
    /// assert_eq!(deque.pop_front(), Some(1));
    /// assert_eq!(deque.pop_front(), Some(2));
    /// assert_eq!(deque.pop_front(), None);
    pub fn pop_front(&mut self) -> Option<T> {
        if self.tail != self.cap() {
            let tail = self.tail;
            self.tail += 1;
            // SAFETY: tail < cap
            unsafe { Some(ptr::read(self.buf_add(tail))) }
        } else if self.head != 0 {
            self.tail = self.cap() - self.head + 1;
            // SAFETY: head > 0 && tail = cap - (head - 1)
            unsafe {
                // ignore the first element because we return it anyway
                self.copy(1, self.tail, self.head - 1);
            }
            self.head = 0;
            // SAFETY: old head was > 0
            unsafe { Some(ptr::read(self.buf_add(0))) }
        } else {
            None
        }
    }

    /// Removes the last element from the deque and returns it, or `None` if the deque is empty.
    ///
    /// Be careful when also using [`pop_front`]. Popping elements from both sides can be very
    /// inefficient. Use [`VecDeque`] if in doubt.
    ///
    /// [`pop_front`]: AltDeque::pop_front
    /// [`VecDeque`]: std::collections::VecDeque
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2]);
    /// assert_eq!(deque.pop_back(), Some(2));
    /// assert_eq!(deque.pop_back(), Some(1));
    /// assert_eq!(deque.pop_back(), None);
    pub fn pop_back(&mut self) -> Option<T> {
        if self.head != 0 {
            self.head -= 1;
            // SAFETY: old head was > 0
            unsafe { Some(ptr::read(self.buf_add(self.head))) }
        } else if self.tail != self.cap() {
            self.head = self.cap() - self.tail - 1;
            // SAFETY: cap - tail < head
            unsafe {
                // ignore the last element because we return it anyway
                self.copy(self.tail, 0, self.head);
            }
            self.tail = self.cap();
            // SAFETY: old tail was < cap
            unsafe { Some(ptr::read(self.buf_add(self.cap() - 1))) }
        } else {
            None
        }
    }

    /// Prepends an element to the front of the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    /// deque.push_front(1);
    /// deque.push_front(2);
    /// deque.push_front(3);
    /// assert_eq!(deque, [3, 2, 1]);
    pub fn push_front(&mut self, value: T) {
        if self.is_full() {
            self.grow();
        }
        self.tail -= 1;
        // SAFETY: old tail was > 0 because buf is not full
        unsafe {
            ptr::write(self.buf_add(self.tail), value);
        }
    }

    /// Appends an element to the back of the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    /// deque.push_back(1);
    /// deque.push_back(2);
    /// deque.push_back(3);
    /// assert_eq!(deque, [1, 2, 3]);
    pub fn push_back(&mut self, value: T) {
        if self.is_full() {
            self.grow();
        }
        // SAFETY: head < tail because buf is not full
        unsafe {
            ptr::write(self.buf_add(self.head), value);
        }
        self.head += 1;
    }

    /// Swaps elements at indices `i` and `j`.
    ///
    /// `i` and `j` may be equal.
    ///
    /// Element at index 0 is the front of the deque.
    ///
    /// # Panics
    ///
    /// Panics if either index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3]);
    /// assert_eq!(deque, [1, 2, 3]);
    /// deque.swap(0, 2);
    /// assert_eq!(deque, [3, 2, 1]);
    /// ```
    pub fn swap(&mut self, i: usize, j: usize) {
        let front_len = self.cap() - self.tail;
        let len = front_len + self.head;
        let i = if i < front_len {
            self.tail + i
        } else if i < len{
            i - front_len
        } else {
            index_out_of_bounds(len, i)
        };
        let j = if j < front_len {
            self.tail + j
        } else if j < len{
            j - front_len
        } else {
            index_out_of_bounds(len, j)
        };
        // SAFETY: these are the same calculations as in get()
        unsafe {
            ptr::swap(self.buf_add(i), self.buf_add(j));
        }
    }

    /// Removes an element from anywhere in the deque and returns it, or `None` if the deque is
    /// empty. The removed element is replaced with the front element.
    ///
    /// This does not preserve ordering, but is *O(1)*.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4]);
    /// assert_eq!(deque.swap_remove_front(2), Some(3));
    /// assert_eq!(deque, [2, 1, 4]);
    /// ```
    pub fn swap_remove_front(&mut self, index: usize) -> Option<T> {
        if index < self.len() {
            self.swap(index, 0);
            self.pop_front()
        } else {
            None
        }
    }

    /// Removes an element from anywhere in the deque and returns it, or `None` if the deque is
    /// empty. The removed element is replaced with the back element.
    ///
    /// This does not preserve ordering, but is *O(1)*.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4]);
    /// assert_eq!(deque.swap_remove_back(1), Some(2));
    /// assert_eq!(deque, [1, 4, 3]);
    /// ```
    pub fn swap_remove_back(&mut self, index: usize) -> Option<T> {
        let len = self.len();
        if index < len {
            self.swap(index, len - 1);
            self.pop_back()
        } else {
            None
        }
    }

    /// Removes and returns the element at `index` from the deque. Returns `None` if `index` is out
    /// of bounds. Either all the elements before or after the removed one will be shifted one
    /// place to close the gap.
    ///
    /// This preserves ordering, but can take up to *O(n)*. If you do not care about ordering use
    /// [`swap_remove_front`] or [`swap_remove_back`].
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// [`swap_remove_back`]: AltDeque::swap_remove_back
    /// [`swap_remove_front`]: AltDeque::swap_remove_front
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4]);
    /// assert_eq!(deque.remove(1), Some(2));
    /// assert_eq!(deque, [1, 3, 4]);
    /// ```
    pub fn remove(&mut self, mut index: usize) -> Option<T> {
        let front_len = self.cap() - self.tail;
        if index < front_len {
            // SAFETY: index < front_len
            let el = unsafe { ptr::read(self.buf_add(self.tail + index)) };
            let new_tail = self.tail + 1;
            // SAFETY: index < front_len -> index elements can be moved from tail to tail + 1
            unsafe {
                self.copy(self.tail, new_tail, index);
            }
            self.tail = new_tail;
            Some(el)
        } else {
            index -= front_len;
            if index < self.head {
                // SAFETY: index < head
                let el = unsafe { ptr::read(self.buf_add(index)) };
                // SAFETY: index < head -> head - index - 1 elements can be moved from index + 1 to index
                unsafe {
                    self.head -= 1;
                    self.copy(index + 1, index, self.head - index);
                }
                Some(el)
            } else {
                None
            }
        }
    }

    /// Inserts an element at `index` within the deque, shifting all elements with indices greater
    /// than or equal to `index` towards the back.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than the deque's length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3]);
    /// deque.insert(1, 5);
    /// assert_eq!(deque, [1, 5, 2, 3]);
    /// ```
    pub fn insert(&mut self, mut index: usize, value: T) {
        if self.is_full() {
            self.grow();
        }

        let front_len = self.cap() - self.tail;
        if index < front_len {
            // SAFETY: tail > 0 (buf !full) && index < front_len -> all elements from tail to tail + index (including)
            // can be moved one to the left. The spot at tail + index is no free and can be written to
            unsafe {
                let new_tail = self.tail - 1;
                self.copy(self.tail, new_tail, index + 1);
                self.tail = new_tail;
                ptr::write(self.buf_add(self.tail + index), value);
            }
        } else {
            index -= front_len;
            if index <= self.head {
                // SAFETY: head < tail (buf !full) && index <= head -> all elements from index to head - index (not including)
                // can be moved one the right. The spot at index is no free and can be written to
                unsafe {
                    self.copy(index, index + 1, self.head - index);
                    self.head += 1;
                    ptr::write(self.buf_add(index), value);
                }
            } else {
                index_out_of_bounds(self.len(), index + front_len);
            }
        }
    }

    /// Splits the deque into two at the given index.
    ///
    /// Returns a newly allocated `AltDeque`. `self` contains elements `[0, at)`,
    /// and the returned deque contains elements `[at, len)`.
    ///
    /// Note that the capacity of `self` does not change.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Panics
    ///
    /// Panics if `at` is greater than the deque's length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4, 5]);
    /// let deque2 = deque.split_off(2);
    /// assert_eq!(deque, [1, 2]);
    /// assert_eq!(deque2, [3, 4, 5]);
    /// ```
    #[must_use = "use `.truncate()` if you don't need the other half"]
    pub fn split_off(&mut self, at: usize) -> Self {
        let front_len = self.cap() - self.tail;
        let len = front_len + self.head;
        if at > len {
            index_out_of_bounds(len, at);
        }

        let other_len = len - at;
        let mut other = Self::with_capacity(other_len);
        // we move the elements to the front stack of other and do not rely on the allocator to return exactly other_len capacity
        if at < front_len {
            // SAFETY:
            // * self and other are different allocations and can not overlap
            // * first all self.head elements from self back stack are moved to other.cap - self.head
            // * then front_len - at elements are moved from self.tail + at to other.cap - other_len
            //   where other.cap - other_len + front_len - at == other.cap - (len - at) + (len - self.head) - at == other.cap - self.head
            // * finally the remaining at elements are moved from self.tail to self.cap - at
            unsafe {
                ptr::copy_nonoverlapping(self.buf_add(0), other.buf_add(other.cap() - self.head), self.head);
                self.head = 0;

                other.tail = other.cap() - other_len;
                ptr::copy_nonoverlapping(self.buf_add(self.tail + at), other.buf_add(other.tail), front_len - at);

                let new_tail = self.cap() - at;
                ptr::copy(self.buf_add(self.tail), self.buf_add(new_tail), at);
                self.tail = new_tail;
            }
        } else {
            // SAFETY: front_len <= at <= len -> 0 <= at - front_len <= head
            unsafe {
                self.head = at - front_len;
                other.tail = other.cap() - other_len;
                ptr::copy_nonoverlapping(self.buf_add(self.head), other.buf_add(other.tail), other_len);
            }
        }

        other
    }

    /// Moves all the elements of `other` into `self`, leaving `other` empty.
    ///
    /// # Panics
    ///
    /// Panics if the new number of elements in self overflows a `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3]);
    /// let mut deque2 = AltDeque::from([4, 5]);
    /// deque.append(&mut deque2);
    /// assert_eq!(deque, [1, 2, 3, 4, 5]);
    /// assert!(deque2.is_empty());
    /// ```
    pub fn append(&mut self, other: &mut Self) {
        let other_front_len = other.cap() - other.tail;
        self.reserve(other_front_len + other.head);
        // SAFETY:
        // * first all other_front_len elements from other.tail are moved after self.head and self.head is updated
        // * then all other.head elements from other are moved after self head and self.head is updated again
        // * finally other is set to empty
        unsafe {
            ptr::copy_nonoverlapping(other.buf_add(other.tail), self.buf_add(self.head), other_front_len);
            self.head += other_front_len;
            ptr::copy_nonoverlapping(other.buf_add(0), self.buf_add(self.head), other.head);
            self.head += other.head;

            other.head = 0;
            other.tail = other.cap();
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `el` for which `f(&el)` returns false. This method
    /// operates in place, visiting each element exactly once in the original order, and preserves
    /// the order of the retained elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4, 5]);
    /// deque.retain(|&el| el % 2 == 0);
    /// assert_eq!(deque, [2, 4]);
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.retain_mut(|elem| f(elem));
    }


    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `el` for which `f(&el)` returns false. This method
    /// operates in place, visiting each element exactly once in the original order, and preserves
    /// the order of the retained elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4, 5]);
    /// deque.retain_mut(|el| { *el += 1; *el % 2 == 0 });
    /// assert_eq!(deque, [2, 4, 6]);
    /// ```
    pub fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        let len = self.len();
        let mut cur = 0;

        // Stage 1: All values are retained.
        loop {
            if cur == len {
                return;
            }
            if !f(&mut self[cur]) {
                cur += 1;
                break;
            }
            cur += 1;
        }
        // Stage 2: Swap retained value into current idx.
        let mut idx = cur - 1; // cur > 0 at this point
        while cur < len {
            if !f(&mut self[cur]) {
                cur += 1;
                continue;
            }
            self.swap(idx, cur);
            cur += 1;
            idx += 1;
        }
        // Stage 3: Truncate all values after idx.
        self.truncate(idx);
    }

    /// Rearranges the internal storage of the deque so it is one contiguous slice, which is then
    /// returned.
    ///
    /// This method does not allocate and does not change the order of the inserted elements. As it
    /// returns a mutable slice, this can be used to sort a deque.
    ///
    /// Once the internal storage is contiguous, the [`as_slices`] and [`as_mut_slices`] methods
    /// will return the entire contents of the deque in a single slice.
    ///
    /// [`as_slices`]: AltDeque::as_slices
    /// [`as_mut_slices`]: AltDeque::as_mut_slices
    ///
    /// # Examples
    ///
    /// Sorting the content of a deque.
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    ///
    /// deque.push_back(2);
    /// deque.push_back(1);
    /// deque.push_front(3);
    ///
    /// // sorting the deque
    /// deque.make_contiguous().sort();
    /// assert_eq!(deque.as_slices(), (&[1, 2, 3] as &[_], &[] as &[_]));
    ///
    /// // sorting it in reverse order
    /// deque.make_contiguous().sort_by(|a, b| b.cmp(a));
    /// assert_eq!(deque.as_slices(), (&[3, 2, 1] as &[_], &[] as &[_]));
    /// ```
    ///
    /// Getting immutable access to the contiguous slice.
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::new();
    ///
    /// deque.push_back(2);
    /// deque.push_back(1);
    /// deque.push_front(3);
    ///
    /// deque.make_contiguous();
    /// if let (slice, &[]) = deque.as_slices() {
    ///     // we can now be sure that `slice` contains all elements of the deque,
    ///     // while still having immutable access to `deque`.
    ///     assert_eq!(deque.len(), slice.len());
    ///     assert_eq!(slice, &[3, 2, 1] as &[_]);
    /// } else {
    ///     unreachable!();
    /// }
    /// ```
    pub fn make_contiguous(&mut self) -> &mut [T] {
        if self.head == 0 {
            return self.as_mut_slices().0;
        }

        let front_len = self.cap() - self.tail;
        let free = self.tail - self.head;

        if front_len == 0 {
            // SAFETY: front is empty we just need to shift back
            // from: ABCDEF...
            // to:   ...ABCDEF
            unsafe {
                self.copy(0, free, self.head);
            }
        } else if free >= self.head {
            // SAFETY: there is enough free space to copy the back
            // first shift the front into position and then copy the back
            // from: EF...ABCD
            //       EF.ABCD..
            // to:   ...ABCDEF
            unsafe {
                self.copy(self.tail, free, front_len);
                ptr::copy_nonoverlapping(self.buf_add(0), self.buf_add(self.cap() - self.head), self.head);
            }
        } else if free >= front_len {
            // SAFETY: there is enough free space to copy the front
            // first shift the back into position and then copy the front
            // from: CDEF...AB
            //       ..CDEF.AB
            // to:   ABCDEF...
            // finally move everything to the front stack
            unsafe {
                self.copy(0, front_len, self.head);
                ptr::copy_nonoverlapping(self.buf_add(self.tail), self.buf_add(0), front_len);
                self.copy(0, free, front_len + self.head);
            }
        } else {
            let mut count = front_len + free;
            let mut left = self.head.saturating_sub(count);
            let mut right = self.head;
            loop {
                for i in left..right {
                    // SAFETY: this just swaps two elements in the range of the buffer
                    // i + count < head + front_len + free = len
                    unsafe {
                        ptr::swap(self.buf_add(i), self.buf_add(i + count));
                    }
                }
                if left != 0 {
                    left = left.saturating_sub(count);
                    right -= count;
                } else if right < count {
                    // head is not a multiple of count, there are now two cases:
                    if right >= count - free {
                        // SAFETY: the last swap swapped all occupied elements and we simply need a final move the remove the gap
                        // example: CDEFGHI..AB
                        //          CDE..ABFGHI
                        //          .AB.CDEFGHI -> move: ..ABCDEFGHI
                        unsafe {
                            self.copy(0, free, right - left);
                        }
                        break;
                    } else {
                        // or we need to do more swaps
                        // example: EFGHIJ.ABCD  +-> more swaps: C.ABDEFGHIJ
                        //          E.ABCDFGHIJ  |               B.ACDEFGHIJ
                        //          D.ABCEFGHIJ -+               A.BCDEFGHIJ -> move: .ABCDEFGHIJ
                        // the swaps will end when count becomes <= right + free (2 <= 1 + 1 in the example)
                        // then the final move is done
                        count -= right;
                    }
                } else {
                    // head is a multiple of count, meaning we are done here
                    // example: CDEFGH.AB
                    //          CDE.ABFGH
                    //          .ABCDEFGH
                    break;
                }
            }
        }

        self.head = 0;
        self.tail = free;

        self.as_mut_slices().0
    }

    /// Rotates the deque `mid` places to the left.
    ///
    /// Equivalently,
    /// - Rotates item `mid` into the first position.
    /// - Pops the first `mid` items and pushes them to the end.
    /// - Rotates `len() - mid` places to the right.
    ///
    /// # Panics
    ///
    /// If `mid` is greater than `len()`. Note that `mid == len()`
    /// does _not_ panic and is a no-op rotation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque: AltDeque<_> = (0..10).collect();
    ///
    /// deque.rotate_left(3);
    /// assert_eq!(deque, [3, 4, 5, 6, 7, 8, 9, 0, 1, 2]);
    /// ```
    pub fn rotate_left(&mut self, mut mid: usize) {
        let front_len = self.cap() - self.tail;
        if mid < front_len {
            // SAFETY: mid < front_len -> we an moce mid elements from tail to head
            unsafe {
                self.copy(self.tail, self.head, mid);
                self.head += mid;
                self.tail += mid;
            }
        } else {
            mid -= front_len;
            if mid <= self.head {
                // SAFETY: mid <= head -> we can move head - mid elements from mid to tail - (head - mid)
                unsafe {
                    let count = self.head - mid;
                    self.head = mid;
                    self.tail -= count;
                    self.copy(mid, self.tail, count);
                }
            } else {
                index_out_of_bounds(self.len(), mid + front_len);
            }
        }
    }

    /// Rotates the deque `k` places to the right.
    ///
    /// Equivalently,
    /// - Rotates the first item into position `k`.
    /// - Pops the last `k` items and pushes them to the front.
    /// - Rotates `len() - k` places to the left.
    ///
    /// # Panics
    ///
    /// If `k` is greater than `len()`. Note that `k == len()`
    /// does _not_ panic and is a no-op rotation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque: AltDeque<_> = (0..10).collect();
    ///
    /// deque.rotate_right(3);
    /// assert_eq!(deque, [7, 8, 9, 0, 1, 2, 3, 4, 5, 6]);
    /// ```
    pub fn rotate_right(&mut self, mut k: usize) {
        if k <= self.head {
            // SAFETY: k <= head -> we can move k elements from head - k to tail - k
            unsafe {
                self.head -= k;
                self.tail -= k;
                self.copy(self.head, self.tail, k);
            }
        } else {
            let front_len = self.cap() - self.tail;
            k -= self.head;
            if k <= front_len {
                // SAFETY: k <= front_len -> we can move front_len - k elements from tail to head
                unsafe {
                    let count = front_len - k;
                    self.copy(self.tail, self.head, count);
                    self.head += count;
                    self.tail += count;
                }
            } else {
                index_out_of_bounds(self.len(), k + self.head);
            }
        }
    }

    /// Binary searches the deque for a given element. This behaves similarly to [`contains`] if
    /// the deque is sorted but is faster.
    ///
    /// If the value is found then [`Result::Ok`] is returned, containing the index of the matching
    /// element. If there are multiple matches, then any one of the matches could be returned.
    /// If the value is not found then [`Result::Err`] is returned, containing the index where a
    /// matching element could be inserted while maintaining sorted order.
    ///
    /// See also [`binary_search_by`], [`binary_search_by_key`], and [`partition_point`].
    ///
    /// [`contains`]: AltDeque::contains
    /// [`binary_search_by`]: AltDeque::binary_search_by
    /// [`binary_search_by_key`]: AltDeque::binary_search_by_key
    /// [`partition_point`]: AltDeque::partition_point
    ///
    /// # Examples
    ///
    /// Looks up a series of four elements. The first is found, with a uniquely determined position;
    /// the second and third are not found; the fourth could match any position in `[1, 4]`.
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let deque = AltDeque::from([0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55]);
    ///
    /// assert_eq!(deque.binary_search(&13),  Ok(9));
    /// assert_eq!(deque.binary_search(&4),   Err(7));
    /// assert_eq!(deque.binary_search(&100), Err(13));
    /// let r = deque.binary_search(&1);
    /// assert!(matches!(r, Ok(1..=4)));
    /// ```
    ///
    /// If you want to insert an item into a sorted deque, while maintaining sort order, consider
    /// using [`partition_point`].
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55]);
    /// let num = 42;
    ///
    /// let idx = deque.partition_point(|&x| x < num);
    /// // The above is equivalent to `let idx = deque.binary_search(&num).unwrap_or_else(|x| x);`
    ///
    /// deque.insert(idx, num);
    /// assert_eq!(deque, &[0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 42, 55]);
    /// ```
    pub fn binary_search(&self, x: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.binary_search_by(|e| e.cmp(x))
    }

    /// Binary searches the deque with a comparator function. This behaves similarly to
    /// [`contains`] if the deque is sorted but is faster.
    ///
    /// The comparator function should implement an order consistent with the sort order of the
    /// deque, returning an order code that indicates whether its argument is `Less`, `Equal` or
    /// `Greater` than the desired target.
    ///
    /// If the value is found then [`Result::Ok`] is returned, containing the index of the matching
    /// element. If there are multiple matches, then any one of the matches could be returned.
    /// If the value is not found then [`Result::Err`] is returned, containing the index where a
    /// matching element could be inserted while maintaining sorted order.
    ///
    /// See also [`binary_search`], [`binary_search_by_key`], and [`partition_point`].
    ///
    /// [`contains`]: AltDeque::contains
    /// [`binary_search`]: AltDeque::binary_search
    /// [`binary_search_by_key`]: AltDeque::binary_search_by_key
    /// [`partition_point`]: AltDeque::partition_point
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// // deque is sorted in reversed order
    /// let deque = AltDeque::from([42, 30, 30, 12, 4, 4, 2, 1, 1, 1]);
    ///
    /// assert_eq!(deque.binary_search_by(|x| 2.cmp(x)),  Ok(6));
    /// assert_eq!(deque.binary_search_by(|x| 21.cmp(x)),   Err(3));
    /// let r = deque.binary_search_by(|x| 1.cmp(x));
    /// assert!(matches!(r, Ok(7..=9)));
    /// ```
    pub fn binary_search_by<'a, F>(&'a self, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> Ordering,
    {
        let (front, back) = self.as_slices();
        let cmp_back = back.first().map(|elem| f(elem));

        if let Some(Ordering::Equal) = cmp_back {
            Ok(front.len())
        } else if let Some(Ordering::Less) = cmp_back {
            back.binary_search_by(f).map(|idx| idx + front.len()).map_err(|idx| idx + front.len())
        } else {
            front.binary_search_by(f)
        }
    }

    /// Binary searches the deque with a key extraction function. This behaves similarly to
    /// [`contains`] if the deque is sorted.
    ///
    /// Assumes that the deque is sorted by the key, for instance with
    /// [`make_contiguous().sort_by_key()`] using the same key extraction function.
    ///
    /// If the value is found then [`Result::Ok`] is returned, containing the index of the matching
    /// element. If there are multiple matches, then any one of the matches could be returned. If
    /// the value is not found then [`Result::Err`] is returned, containing the index where a
    /// matching element could be inserted while maintaining sorted order.
    ///
    /// See also [`binary_search`], [`binary_search_by`], and [`partition_point`].
    ///
    /// [`contains`]: AltDeque::contains
    /// [`binary_search`]: AltDeque::binary_search
    /// [`binary_search_by`]: AltDeque::binary_search_by
    /// [`make_contiguous`]: AltDeque::make_contiguous
    /// [`partition_point`]: AltDeque::partition_point
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let deque = AltDeque::from([(0, 0), (2, 1), (4, 1), (5, 1),
    ///     (3, 1), (1, 2), (2, 3), (4, 5), (5, 8), (3, 13),
    ///     (1, 21), (2, 34), (4, 55)]);
    ///
    /// assert_eq!(deque.binary_search_by_key(&13, |&(_, b)| b),  Ok(9));
    /// assert_eq!(deque.binary_search_by_key(&4, |&(_, b)| b),   Err(7));
    /// assert_eq!(deque.binary_search_by_key(&100, |&(_, b)| b), Err(13));
    /// let r = deque.binary_search_by_key(&1, |&(_, b)| b);
    /// assert!(matches!(r, Ok(1..=4)));
    /// ```
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> B,
        B: Ord,
    {
        self.binary_search_by(|k| f(k).cmp(b))
    }

    /// Returns the index of the partition point according to the given predicate
    /// (the index of the first element of the second partition).
    ///
    /// The deque is assumed to be partitioned according to the given predicate. This means that
    /// all elements for which the predicate returns true are at the start of the deque and all
    /// elements for which the predicate returns false are at the end. For example,
    /// [7, 15, 3, 5, 4, 12, 6] is a partitioned under the predicate x % 2 != 0
    /// (all odd numbers are at the start, all even at the end).
    ///
    /// If the deque is not partitioned, the returned result is unspecified and meaningless,
    /// as this method performs a kind of binary search.
    ///
    /// See also [`binary_search`], [`binary_search_by`], and [`binary_search_by_key`].
    ///
    /// [`binary_search`]: AltDeque::binary_search
    /// [`binary_search_by`]: AltDeque::binary_search_by
    /// [`binary_search_by_key`]: AltDeque::binary_search_by_key
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let deque = AltDeque::from([1, 2, 3, 3, 5, 6, 7]);
    /// let i = deque.partition_point(|&x| x < 5);
    ///
    /// assert_eq!(i, 4);
    /// assert!(deque.iter().take(i).all(|&x| x < 5));
    /// assert!(deque.iter().skip(i).all(|&x| !(x < 5)));
    /// ```
    ///
    /// If you want to insert an item to a sorted deque, while maintaining sort order:
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55]);
    /// let num = 42;
    /// let idx = deque.partition_point(|&x| x < num);
    /// deque.insert(idx, num);
    /// assert_eq!(deque, &[0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 42, 55]);
    /// ```
    pub fn partition_point<P>(&self, mut pred: P) -> usize
    where
        P: FnMut(&T) -> bool,
    {
        let (front, back) = self.as_slices();

        if let Some(true) = back.first().map(|v| pred(v)) {
            back.partition_point(pred) + front.len()
        } else {
            front.partition_point(pred)
        }
    }

    /// Returns a front-to-back iterator over the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let deque = AltDeque::from([1, 2, 3]);
    /// let mut iter = deque.iter();
    /// assert_eq!(iter.next(), Some(&1));
    /// assert_eq!(iter.next(), Some(&2));
    /// assert_eq!(iter.next(), Some(&3));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn iter(&self) -> Iter<'_, T> {
        let (front, back) = self.as_slices();
        front.iter().chain(back.iter())
    }

    /// Returns a front-to-back iterator over the deque that returns mutable references.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3]);
    /// for el in deque.iter_mut() {
    ///     *el += 10;
    /// }
    /// assert_eq!(deque, [11, 12, 13]);
    /// ```
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        let (front, back) = self.as_mut_slices();
        front.iter_mut().chain(back.iter_mut())
    }

    /// Creates an iterator that covers the specified range in the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let deque = AltDeque::from([1, 2, 3, 4, 5, 6]);
    /// assert_eq!(deque.range(1..4).collect::<Vec<_>>(), [&2, &3, &4]);
    /// ```
    pub fn range<R>(&self, range: R) -> Iter<'_, T>
    where
        R: RangeBounds<usize>,
    {
        let Range { start, end } = simplify_range(range, self.len());
        let (front, back) = self.as_slices();
        let front_len = front.len();

        if start >= front_len {
            back[start - front_len..end - front_len].iter().chain(front[..0].iter())
        } else if end <= front_len {
            front[start..end].iter().chain(back[..0].iter())
        } else {
            front[start..].iter().chain(back[..end - front_len].iter())
        }
    }

    /// Creates an iterator that covers the specified mutable range in the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4, 5, 6]);
    /// for el in deque.range_mut(1..4) {
    ///     *el += 10;
    /// }
    /// assert_eq!(deque, [1, 12, 13, 14, 5, 6]);
    /// ```
    pub fn range_mut<R>(&mut self, range: R) -> IterMut<'_, T>
    where
        R: RangeBounds<usize>,
    {
        let Range { start, end } = simplify_range(range, self.len());
        let (front, back) = self.as_mut_slices();
        let front_len = front.len();

        if start >= front_len {
            back[start - front_len..end - front_len].iter_mut().chain(front[..0].iter_mut())
        } else if end <= front_len {
            front[start..end].iter_mut().chain(back[..0].iter_mut())
        } else {
            front[start..].iter_mut().chain(back[..end - front_len].iter_mut())
        }
    }

    /// Removes the specified range from the deque in bulk, returning all removed elements as an
    /// iterator. If the iterator is dropped before being fully consumed, it drops the remaining
    /// removed elements.
    ///
    /// The returned iterator keeps a mutable borrow on the queue to optimize its implementation.
    ///
    /// # Panics
    ///
    /// Panics if the starting point is greater than the end point or if the end point is greater
    /// than the length of the deque.
    ///
    /// # Leaking
    ///
    /// If the returned iterator goes out of scope without being dropped (due to [`mem::forget`],
    /// for example), the deque may have lost and leaked elements arbitrarily, including elements
    /// outside the range and possibly all elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3, 4, 5, 6]);
    /// assert_eq!(deque.drain(1..4).collect::<Vec<_>>(), [2, 3, 4]);
    /// assert_eq!(deque, [1, 5, 6]);
    /// ```
    pub fn drain<R>(&mut self, range: R) -> Drain<T>
    where
        R: RangeBounds<usize>,
    {
        let range = simplify_range(range, self.len());
        let old_head = self.head;
        let old_tail = self.tail;
        self.head = 0;
        self.tail = self.cap();
        Drain::new(self, old_head, old_tail, range)
    }

    #[inline]
    fn cap(&self) -> usize {
        self.buf.capacity()
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.tail == self.head
    }

    #[inline]
    unsafe fn buf_add(&self, offset: usize) -> *mut T {
        self.buf.ptr().add(offset)
    }

    #[inline]
    unsafe fn copy(&mut self, from: usize, to: usize, len: usize) {
        ptr::copy(self.buf_add(from), self.buf_add(to), len);
    }

    /// Double the buffer size. This method is inline(never), so we expect it to only be called in
    /// cold paths. This may panic or abort.
    #[inline(never)]
    fn grow(&mut self) {
        debug_assert!(self.is_full());
        let old_cap = self.cap();
        // this call will panic on overflow or if T is zero-sized
        self.buf.reserve_for_push(old_cap);
        // SAFETY: old_cap is correct
        unsafe { self.handle_capacity_increase(old_cap); }
        debug_assert!(!self.is_full());
    }

    /// Moves the tail to the back to handle the fact that we just reallocated.
    /// Unsafe because it trusts old_cap.
    unsafe fn handle_capacity_increase(&mut self, old_cap: usize) {
        debug_assert!(old_cap >= self.tail);
        debug_assert!(old_cap <= self.cap());

        if old_cap == self.cap() {
            return;
        }

        let growth = self.cap() - old_cap;
        let front_len = old_cap - self.tail;
        let new_tail = self.tail + growth;

        if growth >= front_len {
            // SAFETY: buf was grown by growth >= front_len so we can move front_len elements from tail to tail + growth without overlap
            unsafe {
                ptr::copy_nonoverlapping(self.buf_add(self.tail), self.buf_add(new_tail), front_len);
            }
        } else {
            // SAFETY: buf was grown by growth so we can move front_len elements from tail to tail + growth
            unsafe {
                ptr::copy(self.buf_add(self.tail), self.buf_add(new_tail), front_len);
            }
        }
        self.tail = new_tail;
    }
}

impl<T: Clone> AltDeque<T> {
    /// Modifies the deque in-place so that `len()` is equal to new_len, either by removing excess
    /// elements from the back or by appending clones of `value` to the back.
    ///
    /// # Examples
    ///
    /// ```
    /// # use altdeque::AltDeque;
    /// let mut deque = AltDeque::from([1, 2, 3]);
    ///
    /// deque.resize(2, 5);
    /// assert_eq!(deque, [1, 2]);
    ///
    /// deque.resize(5, 5);
    /// assert_eq!(deque, [1, 2, 5, 5, 5]);
    /// ```
    pub fn resize(&mut self, new_len: usize, value: T) {
        self.resize_with(new_len, || value.clone());
    }
}

impl<T: Clone> Clone for AltDeque<T> {
    fn clone(&self) -> Self {
        let mut deque = Self::with_capacity(self.len());
        if mem::size_of::<T>() == 0 {
            deque.tail = deque.cap() - self.len();
        } else {
            for el in self.iter() {
                deque.push_back(el.clone());
            }
        }
        deque
    }
}

impl<T: fmt::Debug> fmt::Debug for AltDeque<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<T> Default for AltDeque<T> {
    /// Creates an empty deque.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for AltDeque<T> {
    fn drop(&mut self) {
        let (front, back) = self.as_mut_slices();
        unsafe {
            let _back_dropper = Dropper(back);
            // use drop for [T]
            ptr::drop_in_place(front);
        }
        // RawVec handles deallocation
    }
}

impl<T> Extend<T> for AltDeque<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let mut iter = iter.into_iter();
        while let Some(element) = iter.next() {
            if self.is_full() {
                let (lower, _) = iter.size_hint();
                self.reserve(lower.max(1));
            }

            let head = self.head;
            self.head += 1;
            // SAFETY: head < tail because buf is not full
            unsafe {
                ptr::write(self.buf_add(head), element);
            }
        }
    }
}

impl<'a, T: 'a + Copy> Extend<&'a T> for AltDeque<T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.extend(iter.into_iter().copied());
    }
}

impl<T> From<Vec<T>> for AltDeque<T> {
    /// Turns a [`Vec<T>`] into an [`AltDeque<T>`] without reallocating.
    ///
    /// [`AltDeque<T>`]: crate::AltDeque
    fn from(other: Vec<T>) -> Self {
        unsafe {
            // into_raw_parts is currently unstable, see https://github.com/rust-lang/rust/issues/65816
            // let (other_buf, len, capacity) = other.into_raw_parts();

            let mut other = ManuallyDrop::new(other);
            let (other_buf, len, capacity) = (other.as_mut_ptr(), other.len(), other.capacity());
            let buf = RawVec::from_raw_parts(other_buf, capacity);
            Self { buf, head: len, tail: capacity }
        }
    }
}

impl<T> From<AltDeque<T>> for Vec<T> {
    /// Turns an [`AltDeque<T>`] into a [`Vec<T>`].
    ///
    /// This never needs to re-allocate, but does need to do *O(n)* data movement if
    /// the internal front stack is not empty.
    ///
    /// [`AltDeque<T>`]: crate::AltDeque
    fn from(mut other: AltDeque<T>) -> Self {
        if other.tail != other.cap() {
            other.make_contiguous();
            // SAFETY: after the call to make_contiguous all elements are in the front stack and we move them to the left
            unsafe {
                other.copy(other.tail, 0, other.cap() - other.tail);
            }
        }

        // SAFETY: we construct a Vec from a valid ptr, capacity und length
        unsafe {
            let other = ManuallyDrop::new(other);
            let buf = other.buf.ptr();
            let len = other.len();
            let cap = other.cap();
            Vec::from_raw_parts(buf, len, cap)
        }
    }
}

impl<T, const N: usize> From<[T; N]> for AltDeque<T> {
    /// Converts a `[T; N]` into a `AltDeque<T>`.
    ///
    /// [`AltDeque<T>`]: crate::AltDeque
    fn from(arr: [T; N]) -> Self {
        let mut deque = AltDeque::with_capacity(N);
        let arr = ManuallyDrop::new(arr);
        deque.tail = deque.cap() - N;
        if mem::size_of::<T>() != 0 {
            // SAFETY: AltDeque::with_capacity ensures that there is enough capacity.
            unsafe {
                ptr::copy_nonoverlapping(arr.as_ptr(), deque.buf_add(deque.tail), N);
            }
        }
        deque
    }
}

impl<T, const N: usize, const M: usize> From<([T; N], [T; M])> for AltDeque<T> {
    /// Creates a deque from a tuple of arrays. The first one will be used as the
    /// internal front stack and the second as the back stack.
    fn from(tuple: ([T; N], [T; M])) -> Self {
        let (front, back) = tuple;
        let mut deque = AltDeque::with_capacity(N + M);

        if mem::size_of::<T>() != 0 {
            let front = ManuallyDrop::new(front);
            let back = ManuallyDrop::new(back);
            deque.tail = deque.cap() - N;
            deque.head = M;
            // SAFETY: AltDeque::with_capacity ensures that there is enough capacity.
            unsafe {
                debug_assert!(deque.head <= deque.tail);
                ptr::copy_nonoverlapping(front.as_ptr(), deque.buf_add(deque.tail), N);
                ptr::copy_nonoverlapping(back.as_ptr(), deque.buf_add(0), M);
            }
        }
        deque
    }
}

impl<T> FromIterator<T> for AltDeque<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();
        let mut deque = Self::with_capacity(lower);
        deque.extend(iter);
        deque
    }
}

impl<T: Hash> Hash for AltDeque<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // write_length_prefix is currently unstable, see https://github.com/rust-lang/rust/issues/96762
        // state.write_length_prefix(self.len());

        state.write_usize(self.len());
        self.iter().for_each(|elem| elem.hash(state));
    }
}

impl<T> Index<usize> for AltDeque<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &T {
        self.get(index).unwrap_or_else(|| index_out_of_bounds(self.len(), index))
    }
}

impl<T> IndexMut<usize> for AltDeque<T> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut T {
        let len = self.len();
        self.get_mut(index).unwrap_or_else(|| index_out_of_bounds(len, index))
    }
}

impl<T> IntoIterator for AltDeque<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    /// Consumes the deque into a front-to-back iterator yielding elements by value.
    fn into_iter(self) -> IntoIter<T> {
        IntoIter::new(self)
    }
}

impl<'a, T> IntoIterator for &'a AltDeque<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut AltDeque<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

impl<T: PartialOrd> PartialOrd for AltDeque<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T: Ord> Ord for AltDeque<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<T: PartialEq> PartialEq for AltDeque<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let (sa, sb) = self.as_slices();
        let (oa, ob) = other.as_slices();
        if sa.len() == oa.len() {
            sa == oa && sb == ob
        } else if sa.len() < oa.len() {
            // Always divisible in three sections, for example:
            // self:  [a b c|d e f]
            // other: [0 1 2 3|4 5]
            // front = 3, mid = 1,
            // [a b c] == [0 1 2] && [d] == [3] && [e f] == [4 5]
            let front = sa.len();
            let mid = oa.len() - front;

            let (oa_front, oa_mid) = oa.split_at(front);
            let (sb_mid, sb_back) = sb.split_at(mid);

            sa == oa_front && sb_mid == oa_mid && sb_back == ob
        } else {
            let front = oa.len();
            let mid = sa.len() - front;

            let (sa_front, sa_mid) = sa.split_at(front);
            let (ob_mid, ob_back) = ob.split_at(mid);

            sa_front == oa && sa_mid == ob_mid && sb == ob_back
        }
    }
}

impl<T: Eq> Eq for AltDeque<T> {}

__impl_slice_eq! { [] AltDeque<T>, Vec<U>, }
__impl_slice_eq! { [] AltDeque<T>, &[U], }
__impl_slice_eq! { [] AltDeque<T>, &mut [U], }
__impl_slice_eq! { [const N: usize] AltDeque<T>, [U; N], }
__impl_slice_eq! { [const N: usize] AltDeque<T>, &[U; N], }
__impl_slice_eq! { [const N: usize] AltDeque<T>, &mut [U; N], }

fn index_out_of_bounds(len: usize, index: usize) -> ! {
    panic!("index out of bounds: the len is {} but the index is {}", len, index);
}

fn simplify_range(range: impl RangeBounds<usize>, len: usize) -> Range<usize> {
    // we later check for start > end so ignore here if start > len
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(&i) => i,
        Bound::Excluded(&i) => i.checked_add(1).expect("range start Bound::Excluded(usize::MAX) is > usize::MAX"),
    };
    let end = match range.end_bound() {
        Bound::Unbounded => len,
        Bound::Excluded(&i) if i <= len => i,
        Bound::Included(&i) if i < len => i + 1,
        bound => panic!("range end {:?} should be <= length {}", bound, len),
    };
    if start > end {
        panic!(
            "range start {:?} should be <= range end {:?}",
            range.start_bound(),
            range.end_bound()
        );
    }
    start..end
}
