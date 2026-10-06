//! GC reference handle — a safe(r) wrapper around a pointer to a GC-managed object.
//!
//! `GcRef<T>` is the primary way user code holds references to GC-managed objects.
//! It stores a raw pointer to the object on the GC heap (after the [`GcHeader`]).

use crate::header::GcHeader;
use std::fmt;
use std::marker::PhantomData;

/// A reference to a GC-managed object of type `T`.
///
/// This is a lightweight handle (pointer-sized) that can be stored in other
/// GC-managed objects and will be updated during moving collections.
///
/// # Null References
///
/// `GcRef` can represent null (uninitialized) references. Always check
/// `is_null()` before dereferencing.
pub struct GcRef<T> {
    /// Raw pointer to the object payload (after the GcHeader).
    /// Null means "no reference".
    ptr: *mut u8,
    _phantom: PhantomData<T>,
}

impl<T> GcRef<T> {
    /// Create a null GC reference.
    #[inline]
    pub const fn null() -> Self {
        Self { ptr: std::ptr::null_mut(), _phantom: PhantomData }
    }

    /// Create a GC reference from a raw pointer to the object payload.
    ///
    /// # Safety
    /// The pointer must point to a valid, GC-allocated object of type `T`,
    /// or be null.
    #[inline]
    pub unsafe fn from_raw(ptr: *mut u8) -> Self {
        Self { ptr, _phantom: PhantomData }
    }

    /// Check if this reference is null.
    #[inline]
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// Get the raw pointer to the object payload.
    #[inline]
    pub fn as_raw(&self) -> *mut u8 {
        self.ptr
    }

    /// Get a reference to the GC header for this object.
    ///
    /// # Safety
    /// The reference must not be null and must point to a valid GC allocation.
    #[inline]
    pub unsafe fn header(&self) -> &GcHeader {
        debug_assert!(!self.is_null(), "cannot get header of null GcRef");
        // SAFETY: The GcHeader is located immediately before the payload.
        unsafe { &*(self.ptr.sub(std::mem::size_of::<GcHeader>()) as *const GcHeader) }
    }

    /// Get a mutable reference to the GC header for this object.
    ///
    /// # Safety
    /// The reference must not be null and must point to a valid GC allocation.
    #[inline]
    pub unsafe fn header_mut(&mut self) -> &mut GcHeader {
        debug_assert!(!self.is_null(), "cannot get header of null GcRef");
        // SAFETY: The GcHeader is located immediately before the payload.
        unsafe { &mut *(self.ptr.sub(std::mem::size_of::<GcHeader>()) as *mut GcHeader) }
    }

    /// Get a reference to the payload as type `T`.
    ///
    /// # Safety
    /// - The reference must not be null.
    /// - The payload must be a valid `T`.
    /// - The usual Rust aliasing rules apply.
    #[inline]
    pub unsafe fn as_ref(&self) -> &T {
        debug_assert!(!self.is_null(), "cannot deref null GcRef");
        // SAFETY: Caller guarantees validity.
        unsafe { &*(self.ptr as *const T) }
    }

    /// Get a mutable reference to the payload as type `T`.
    ///
    /// # Safety
    /// - The reference must not be null.
    /// - The payload must be a valid `T`.
    /// - The caller must have exclusive access.
    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut T {
        debug_assert!(!self.is_null(), "cannot deref null GcRef");
        // SAFETY: Caller guarantees validity and exclusivity.
        unsafe { &mut *(self.ptr as *mut T) }
    }

    /// Update the internal pointer. Used by the GC during moving collections.
    ///
    /// # Safety
    /// The new pointer must point to a valid GC allocation of the same type.
    #[inline]
    pub unsafe fn update_ptr(&mut self, new_ptr: *mut u8) {
        self.ptr = new_ptr;
    }
}

// GcRef is Copy — it's just a pointer.
impl<T> Copy for GcRef<T> {}
impl<T> Clone for GcRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for GcRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

impl<T> Eq for GcRef<T> {}

impl<T> fmt::Debug for GcRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_null() { write!(f, "GcRef(null)") } else { write!(f, "GcRef({:p})", self.ptr) }
    }
}

impl<T> Default for GcRef<T> {
    fn default() -> Self {
        Self::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_ref() {
        let r: GcRef<u64> = GcRef::null();
        assert!(r.is_null());
    }

    #[test]
    fn test_ref_equality() {
        let a: GcRef<u64> = GcRef::null();
        let b: GcRef<u64> = GcRef::null();
        assert_eq!(a, b);
    }

    #[test]
    fn test_ref_is_copy() {
        let a: GcRef<u64> = GcRef::null();
        let b = a;
        assert_eq!(a, b); // a is still valid because GcRef is Copy
    }

    #[test]
    fn test_from_raw_roundtrip() {
        let mut value: u64 = 42;
        let ptr = &mut value as *mut u64 as *mut u8;
        let gc_ref = unsafe { GcRef::<u64>::from_raw(ptr) };
        assert!(!gc_ref.is_null());
        assert_eq!(gc_ref.as_raw(), ptr);
    }
}
