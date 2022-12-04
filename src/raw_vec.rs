//! This a modified version of the RawVec implementation from the standard library that does not
//! use any unstable features.
//! See <https://github.com/rust-lang/rust/blob/master/library/alloc/src/raw_vec.rs>
//!
//! credits to contributors of the [rust project](https://github.com/rust-lang/rust/)

use std::alloc::{self, handle_alloc_error, Layout, LayoutError};
use std::boxed::Box;
use std::cmp;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::ptr::NonNull;
use std::slice;

pub(crate) enum TryReserveError {
    CapacityOverflow,
    AllocError(Layout),
}

/// A low-level utility for more ergonomically allocating, reallocating, and deallocating
/// a buffer of memory on the heap without having to worry about all the corner cases
/// involved. This type is excellent for building your own data structures like Vec and VecDeque.
/// In particular:
///
/// * Produces `NonNull::dangling()` on zero-sized types.
/// * Produces `NonNull::dangling()` on zero-length allocations.
/// * Avoids freeing `NonNull::dangling()`.
/// * Catches all overflows in capacity computations (promotes them to "capacity overflow" panics).
/// * Guards against 32-bit systems allocating more than isize::MAX bytes.
/// * Guards against overflowing your length.
/// * Calls `handle_alloc_error` for fallible allocations.
/// * Contains a `ptr::NonNull` and thus endows the user with all related benefits.
/// * Uses the excess returned from the allocator to use the largest available capacity.
///
/// This type does not in anyway inspect the memory that it manages. When dropped it *will*
/// free its memory, but it *won't* try to drop its contents. It is up to the user of `RawVec`
/// to handle the actual things *stored* inside of a `RawVec`.
///
/// Note that the excess of a zero-sized types is always infinite, so `capacity()` always returns
/// `usize::MAX`. This means that you need to be careful when round-tripping this type with a
/// `Box<[T]>`, since `capacity()` won't yield the length.
#[allow(missing_debug_implementations)]
pub(crate) struct RawVec<T> {
    ptr: NonNull<T>,
    cap: usize,
    _marker: PhantomData<T>,
}

impl<T> RawVec<T> {
    // Tiny Vecs are dumb. Skip to:
    // - 8 if the element size is 1, because any heap allocators is likely
    //   to round up a request of less than 8 bytes to at least 8 bytes.
    // - 4 if elements are moderate-sized (<= 1 KiB).
    // - 1 otherwise, to avoid wasting too much space for very short Vecs.
    pub(crate) const MIN_NON_ZERO_CAP: usize = if mem::size_of::<T>() == 1 {
        8
    } else if mem::size_of::<T>() <= 1024 {
        4
    } else {
        1
    };

    /// Creates the biggest possible `RawVec` (on the system heap)
    /// without allocating. If `T` has positive size, then this makes a
    /// `RawVec` with capacity `0`. If `T` is zero-sized, then it makes a
    /// `RawVec` with capacity `usize::MAX`. Useful for implementing
    /// delayed allocation.
    #[must_use]
    pub const fn new() -> Self {
        // `cap: 0` means "unallocated". zero-sized types are ignored.
        Self { ptr: NonNull::dangling(), cap: 0, _marker: PhantomData }
    }

    /// Creates a `RawVec` (on the system heap) with exactly the
    /// capacity and alignment requirements for a `[T; capacity]`. This is
    /// equivalent to calling `RawVec::new` when `capacity` is `0` or `T` is
    /// zero-sized. Note that if `T` is zero-sized this means you will
    /// *not* get a `RawVec` with the requested capacity.
    ///
    /// # Panics
    ///
    /// Panics if the requested capacity exceeds `isize::MAX` bytes.
    ///
    /// # Aborts
    ///
    /// Aborts on OOM.
    #[must_use]
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::allocate(capacity)
    }

    /// Converts the entire buffer into `Box<[MaybeUninit<T>]>` with the specified `len`.
    ///
    /// Note that this will correctly reconstitute any `cap` changes
    /// that may have been performed. (See description of type for details.)
    ///
    /// # Safety
    ///
    /// * `len` must be greater than or equal to the most recently requested capacity, and
    /// * `len` must be less than or equal to `self.capacity()`.
    ///
    /// Note, that the requested capacity and `self.capacity()` could differ, as
    /// an allocator could overallocate and return a greater memory block than requested.
    #[allow(unused)]
    pub unsafe fn into_box(self, len: usize) -> Box<[MaybeUninit<T>]> {
        // Sanity-check one half of the safety requirement (we cannot check the other half).
        debug_assert!(
            len <= self.capacity(),
            "`len` must be smaller than or equal to `self.capacity()`"
        );

        let me = ManuallyDrop::new(self);
        unsafe {
            let slice = slice::from_raw_parts_mut(me.ptr() as *mut MaybeUninit<T>, len);
            Box::from_raw(slice)
        }
    }

    fn allocate(capacity: usize) -> Self {
        // Don't allocate here because `Drop` will not deallocate when `capacity` is 0.
        if mem::size_of::<T>() == 0 || capacity == 0 {
            Self::new()
        } else {
            // We avoid `unwrap_or_else` here because it bloats the amount of
            // LLVM IR generated.
            let layout = match Layout::array::<T>(capacity) {
                Ok(layout) => layout,
                Err(_) => capacity_overflow(),
            };
            match alloc_guard(layout.size()) {
                Ok(_) => {}
                Err(_) => capacity_overflow(),
            }
            let ptr = unsafe { alloc::alloc(layout) };
            // If allocation fails, `new_ptr` will be null, in which case we abort.
            let ptr = match NonNull::new(ptr as *mut T) {
                Some(p) => p,
                None => alloc::handle_alloc_error(layout),
            };

            // Allocators currently return a `NonNull<[u8]>` whose length
            // matches the size requested. If that ever changes, the capacity
            // here should change to `ptr.len() / mem::size_of::<T>()`.
            Self {
                ptr,
                cap: capacity,
                _marker: PhantomData,
            }
        }
    }

    /// Reconstitutes a `RawVec` from a pointer and capacity.
    ///
    /// # Safety
    ///
    /// The `ptr` must be allocated with the given `capacity`.
    /// The `capacity` cannot exceed `isize::MAX` for sized types. (only a concern on 32-bit
    /// systems). ZST vectors may have a capacity up to `usize::MAX`.
    /// If the `ptr` and `capacity` come from a `RawVec`, then this is
    /// guaranteed.
    #[inline]
    pub unsafe fn from_raw_parts(ptr: *mut T, capacity: usize) -> Self {
        Self { ptr: unsafe { NonNull::new_unchecked(ptr) }, cap: capacity, _marker: PhantomData }
    }

    /// Gets a raw pointer to the start of the allocation. Note that this is
    /// `NonNull::dangling()` if `capacity == 0` or `T` is zero-sized. In the former case, you must
    /// be careful.
    #[inline]
    pub fn ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    /// Gets the capacity of the allocation.
    ///
    /// This will always be `usize::MAX` if `T` is zero-sized.
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        if mem::size_of::<T>() == 0 { usize::MAX } else { self.cap }
    }

    fn current_memory(&self) -> Option<(NonNull<u8>, Layout)> {
        if mem::size_of::<T>() == 0 || self.cap == 0 {
            None
        } else {
            // We have an allocated chunk of memory, so we can bypass runtime
            // checks to get our current layout.
            unsafe {
                let layout = Layout::array::<T>(self.cap).unwrap_unchecked();
                Some((self.ptr.cast(), layout))
            }
        }
    }

    /// Ensures that the buffer contains at least enough space to hold `len +
    /// additional` elements. If it doesn't already have enough capacity, will
    /// reallocate enough space plus comfortable slack space to get amortized
    /// *O*(1) behavior. Will limit this behavior if it would needlessly cause
    /// itself to panic.
    ///
    /// If `len` exceeds `self.capacity()`, this may fail to actually allocate
    /// the requested space. This is not really unsafe, but the unsafe
    /// code *you* write that relies on the behavior of this function may break.
    ///
    /// This is ideal for implementing a bulk-push operation like `extend`.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// # Aborts
    ///
    /// Aborts on OOM.
    #[inline]
    pub fn reserve(&mut self, len: usize, additional: usize) {
        // Callers expect this function to be very cheap when there is already sufficient capacity.
        // Therefore, we move all the resizing and error-handling logic from grow_amortized and
        // handle_reserve behind a call, while making sure that this function is likely to be
        // inlined as just a comparison and a call if the comparison fails.
        #[cold]
        fn do_reserve_and_handle<T>(
            slf: &mut RawVec<T>,
            len: usize,
            additional: usize,
        ) {
            handle_reserve(slf.grow_amortized(len, additional));
        }

        if self.needs_to_grow(len, additional) {
            do_reserve_and_handle(self, len, additional);
        }
    }

    /// A specialized version of `reserve()` used only by the hot and
    /// oft-instantiated `Vec::push()`, which does its own capacity check.
    #[inline(never)]
    pub fn reserve_for_push(&mut self, len: usize) {
        handle_reserve(self.grow_amortized(len, 1));
    }

    /// The same as `reserve`, but returns on errors instead of panicking or aborting.
    #[allow(unused)]
    pub fn try_reserve(&mut self, len: usize, additional: usize) -> Result<(), TryReserveError> {
        if self.needs_to_grow(len, additional) {
            self.grow_amortized(len, additional)
        } else {
            Ok(())
        }
    }

    /// Ensures that the buffer contains at least enough space to hold `len +
    /// additional` elements. If it doesn't already, will reallocate the
    /// minimum possible amount of memory necessary. Generally this will be
    /// exactly the amount of memory necessary, but in principle the allocator
    /// is free to give back more than we asked for.
    ///
    /// If `len` exceeds `self.capacity()`, this may fail to actually allocate
    /// the requested space. This is not really unsafe, but the unsafe code
    /// *you* write that relies on the behavior of this function may break.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// # Aborts
    ///
    /// Aborts on OOM.
    pub fn reserve_exact(&mut self, len: usize, additional: usize) {
        handle_reserve(self.try_reserve_exact(len, additional));
    }

    /// The same as `reserve_exact`, but returns on errors instead of panicking or aborting.
    pub fn try_reserve_exact(
        &mut self,
        len: usize,
        additional: usize,
    ) -> Result<(), TryReserveError> {
        if self.needs_to_grow(len, additional) { self.grow_exact(len, additional) } else { Ok(()) }
    }

    /// Shrinks the buffer down to the specified capacity. If the given amount
    /// is 0, actually completely deallocates.
    ///
    /// # Panics
    ///
    /// Panics if the given amount is *larger* than the current capacity.
    ///
    /// # Aborts
    ///
    /// Aborts on OOM.
    pub fn shrink_to_fit(&mut self, cap: usize) {
        handle_reserve(self.shrink(cap));
    }

    /// Returns if the buffer needs to grow to fulfill the needed extra capacity.
    /// Mainly used to make inlining reserve-calls possible without inlining `grow`.
    fn needs_to_grow(&self, len: usize, additional: usize) -> bool {
        additional > self.capacity().wrapping_sub(len)
    }

    fn set_ptr_and_cap(&mut self, ptr: NonNull<u8>, cap: usize) {
        // Allocators currently return a `NonNull<[u8]>` whose length matches
        // the size requested. If that ever changes, the capacity here should
        // change to `ptr.len() / mem::size_of::<T>()`.
        self.ptr = unsafe { NonNull::new_unchecked(ptr.cast().as_ptr()) };
        self.cap = cap;
    }

    // This method is usually instantiated many times. So we want it to be as
    // small as possible, to improve compile times. But we also want as much of
    // its contents to be statically computable as possible, to make the
    // generated code run faster. Therefore, this method is carefully written
    // so that all of the code that depends on `T` is within it, while as much
    // of the code that doesn't depend on `T` as possible is in functions that
    // are non-generic over `T`.
    fn grow_amortized(&mut self, len: usize, additional: usize) -> Result<(), TryReserveError> {
        // This is ensured by the calling contexts.
        debug_assert!(additional > 0);

        if mem::size_of::<T>() == 0 {
            // Since we return a capacity of `usize::MAX` when `elem_size` is
            // 0, getting to here necessarily means the `RawVec` is overfull.
            return Err(TryReserveError::CapacityOverflow);
        }

        // Nothing we can really do about these checks, sadly.
        let required_cap = len.checked_add(additional).ok_or(TryReserveError::CapacityOverflow)?;

        // This guarantees exponential growth. The doubling cannot overflow
        // because `cap <= isize::MAX` and the type of `cap` is `usize`.
        let cap = cmp::max(self.cap * 2, required_cap);
        let cap = cmp::max(Self::MIN_NON_ZERO_CAP, cap);

        let new_layout = Layout::array::<T>(cap);

        // `finish_grow` is non-generic over `T`.
        let ptr = finish_grow(new_layout, self.current_memory())?;
        self.set_ptr_and_cap(ptr, cap);
        Ok(())
    }

    // The constraints on this method are much the same as those on
    // `grow_amortized`, but this method is usually instantiated less often so
    // it's less critical.
    fn grow_exact(&mut self, len: usize, additional: usize) -> Result<(), TryReserveError> {
        if mem::size_of::<T>() == 0 {
            // Since we return a capacity of `usize::MAX` when the type size is
            // 0, getting to here necessarily means the `RawVec` is overfull.
            return Err(TryReserveError::CapacityOverflow);
        }

        let cap = len.checked_add(additional).ok_or(TryReserveError::CapacityOverflow)?;
        let new_layout = Layout::array::<T>(cap);

        // `finish_grow` is non-generic over `T`.
        let ptr = finish_grow(new_layout, self.current_memory())?;
        self.set_ptr_and_cap(ptr, cap);
        Ok(())
    }

    fn shrink(&mut self, cap: usize) -> Result<(), TryReserveError> {
        assert!(cap <= self.capacity(), "Tried to shrink to a larger capacity");

        let (ptr, layout) = if let Some(mem) = self.current_memory() { mem } else { return Ok(()) };

        let ptr = unsafe {
            // `Layout::array` cannot overflow here because it would have
            // overflowed earlier when capacity was larger.
            let new_layout = Layout::array::<T>(cap).unwrap_unchecked();
            let ptr = alloc::realloc(ptr.as_ptr(), layout, new_layout.size());
            NonNull::new(ptr).unwrap()
        };
        self.set_ptr_and_cap(ptr, cap);
        Ok(())
    }
}

// This function is outside `RawVec` to minimize compile times. See the comment above `RawVec::grow_amortized` for details.
#[inline(never)]
fn finish_grow(
    new_layout: Result<Layout, LayoutError>,
    current_memory: Option<(NonNull<u8>, Layout)>,
) -> Result<NonNull<u8>, TryReserveError> {
    // Check for the error here to minimize the size of `RawVec::grow_*`.
    let new_layout = new_layout.map_err(|_| TryReserveError::CapacityOverflow)?;

    alloc_guard(new_layout.size())?;

    let ptr = if let Some((ptr, old_layout)) = current_memory {
        debug_assert_eq!(old_layout.align(), new_layout.align());
        unsafe {
            // The allocator checks for alignment equality
            // intrinsics::assume(old_layout.align() == new_layout.align());
            alloc::realloc(ptr.as_ptr(), old_layout, new_layout.size())
        }
    } else {
        unsafe { alloc::alloc(new_layout) }
    };

    NonNull::new(ptr).ok_or(TryReserveError::AllocError(new_layout))
}

impl<T> Drop for RawVec<T> {
    /// Frees the memory owned by the `RawVec` *without* trying to drop its contents.
    fn drop(&mut self) {
        if let Some((ptr, layout)) = self.current_memory() {
            unsafe { alloc::dealloc(ptr.as_ptr(), layout) }
        }
    }
}

// Central function for reserve error handling.
#[inline]
fn handle_reserve(result: Result<(), TryReserveError>) {
    match result {
        Err(TryReserveError::CapacityOverflow) => capacity_overflow(),
        Err(TryReserveError::AllocError(layout)) => handle_alloc_error(layout),
        Ok(()) => { /* yay */ }
    }
}

// We need to guarantee the following:
// * We don't ever allocate `> isize::MAX` byte-size objects.
// * We don't overflow `usize::MAX` and actually allocate too little.
//
// On 64-bit we just need to check for overflow since trying to allocate
// `> isize::MAX` bytes will surely fail. On 32-bit and 16-bit we need to add
// an extra guard for this in case we're running on a platform which can use
// all 4GB in user-space, e.g., PAE or x32.

#[inline]
fn alloc_guard(alloc_size: usize) -> Result<(), TryReserveError> {
    if usize::BITS < 64 && alloc_size > isize::MAX as usize {
        Err(TryReserveError::CapacityOverflow)
    } else {
        Ok(())
    }
}

// One central function responsible for reporting capacity overflows. This'll
// ensure that the code generation related to these panics is minimal as there's
// only one location which panics rather than a bunch throughout the module.
fn capacity_overflow() -> ! {
    panic!("capacity overflow");
}
