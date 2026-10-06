//! Object header for GC-managed allocations.
//!
//! Every object on the GC heap is prefixed with a [`GcHeader`] that stores:
//! - The size of the allocation (including the header)
//! - A type tag for GC tracing dispatch
//! - Flags (forwarded state, pinned, etc.)
//!
//! During a nursery scavenge (Cheney's algorithm), the header is overwritten
//! with a forwarding pointer when an object is copied to to-space.

use std::fmt;

/// Minimum alignment for all GC-allocated objects.
pub const GC_ALIGN: usize = 8;

/// Size of the object header in bytes.
pub const HEADER_SIZE: usize = std::mem::size_of::<GcHeader>();

/// Flag: object has been forwarded during scavenge.
const FLAG_FORWARDED: u16 = 1 << 0;
/// Flag: object is pinned and must not be moved.
const FLAG_PINNED: u16 = 1 << 1;
/// Flag: object has been marked during mark phase.
const FLAG_MARKED: u16 = 1 << 2;
/// Flag: object has been promoted to old generation.
const FLAG_TENURED: u16 = 1 << 3;

/// Type tag identifying the kind of GC-managed object.
///
/// The GC uses this to dispatch tracing — each type tag corresponds to a
/// specific `Trace` implementation that knows which fields contain references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum TypeTag {
    /// A raw, untyped allocation (for internal use).
    Raw = 0,
    /// A JavaScript string (WTF-8 encoded).
    String = 1,
    /// A JavaScript object (ordinary or exotic).
    Object = 2,
    /// A JavaScript array.
    Array = 3,
    /// A JavaScript function.
    Function = 4,
    /// A JavaScript regular expression.
    RegExp = 5,
    /// An ArrayBuffer backing store.
    ArrayBuffer = 6,
    /// A Symbol description.
    Symbol = 7,
    /// A BigInt value.
    BigInt = 8,
    /// A closure environment (scope chain).
    Environment = 9,
    /// A compiled bytecode chunk.
    Bytecode = 10,
    /// A shape / hidden class.
    Shape = 11,
    /// A property storage array.
    PropertyStorage = 12,
}

/// Object header prepended to every GC-allocated object.
///
/// Layout (8 bytes total):
/// ```text
/// ┌──────────────────┬──────────────────┬──────────────────┐
/// │   size: u32      │   type_tag: u16  │   flags: u16     │
/// │   (incl. header) │                  │                  │
/// └──────────────────┴──────────────────┴──────────────────┘
/// ```
///
/// When forwarded during scavenge, the 8 bytes immediately after the header
/// are overwritten with the forwarding address (a `*mut u8`).
#[derive(Clone, Copy)]
#[repr(C)]
pub struct GcHeader {
    /// Total size of the allocation in bytes, including this header.
    /// Maximum single allocation: ~4 GB (sufficient; larger objects go to
    /// the Large Object Space).
    pub size: u32,
    /// Type tag for GC tracing dispatch.
    pub type_tag: u16,
    /// Bitfield of flags.
    pub flags: u16,
}

impl GcHeader {
    /// Create a new header for an allocation of the given total size and type.
    #[inline]
    pub fn new(total_size: u32, type_tag: TypeTag) -> Self {
        Self { size: total_size, type_tag: type_tag as u16, flags: 0 }
    }

    /// Returns the total size of this allocation including the header.
    #[inline]
    pub fn total_size(&self) -> usize {
        self.size as usize
    }

    /// Returns the size of the payload (excluding the header).
    #[inline]
    pub fn payload_size(&self) -> usize {
        self.size as usize - HEADER_SIZE
    }

    /// Returns a pointer to the payload (the data after the header).
    ///
    /// # Safety
    /// The caller must ensure `self` points to a valid GC allocation.
    #[inline]
    pub unsafe fn payload_ptr(&self) -> *const u8 {
        // SAFETY: Header is always followed by payload in a valid GC allocation.
        unsafe { (self as *const GcHeader).add(1) as *const u8 }
    }

    /// Returns a mutable pointer to the payload.
    ///
    /// # Safety
    /// The caller must ensure `self` points to a valid, mutable GC allocation.
    #[inline]
    pub unsafe fn payload_ptr_mut(&mut self) -> *mut u8 {
        // SAFETY: Header is always followed by payload in a valid GC allocation.
        unsafe { (self as *mut GcHeader).add(1) as *mut u8 }
    }

    /// Returns the type tag as a `TypeTag` enum.
    #[inline]
    pub fn tag(&self) -> TypeTag {
        // SAFETY: We only write valid TypeTag values.
        // Fall back to Raw for unknown tags (forward compatibility).
        match self.type_tag {
            0 => TypeTag::Raw,
            1 => TypeTag::String,
            2 => TypeTag::Object,
            3 => TypeTag::Array,
            4 => TypeTag::Function,
            5 => TypeTag::RegExp,
            6 => TypeTag::ArrayBuffer,
            7 => TypeTag::Symbol,
            8 => TypeTag::BigInt,
            9 => TypeTag::Environment,
            10 => TypeTag::Bytecode,
            11 => TypeTag::Shape,
            12 => TypeTag::PropertyStorage,
            _ => TypeTag::Raw,
        }
    }

    // ── Flag queries ──

    /// Check if this object has been forwarded during scavenge.
    #[inline]
    pub fn is_forwarded(&self) -> bool {
        self.flags & FLAG_FORWARDED != 0
    }

    /// Check if this object is pinned (must not be moved).
    #[inline]
    pub fn is_pinned(&self) -> bool {
        self.flags & FLAG_PINNED != 0
    }

    /// Check if this object has been marked during a mark phase.
    #[inline]
    pub fn is_marked(&self) -> bool {
        self.flags & FLAG_MARKED != 0
    }

    /// Check if this object has been promoted to the old generation.
    #[inline]
    pub fn is_tenured(&self) -> bool {
        self.flags & FLAG_TENURED != 0
    }

    // ── Flag mutations ──

    /// Set the marked flag.
    #[inline]
    pub fn set_marked(&mut self) {
        self.flags |= FLAG_MARKED;
    }

    /// Clear the marked flag.
    #[inline]
    pub fn clear_marked(&mut self) {
        self.flags &= !FLAG_MARKED;
    }

    /// Set the pinned flag.
    #[inline]
    pub fn set_pinned(&mut self) {
        self.flags |= FLAG_PINNED;
    }

    /// Mark this object as tenured (promoted to old generation).
    #[inline]
    pub fn set_tenured(&mut self) {
        self.flags |= FLAG_TENURED;
    }

    // ── Forwarding (used during nursery scavenge) ──

    /// Set the forwarding address. Marks the object as forwarded.
    ///
    /// After this call, the first `size_of::<*mut u8>()` bytes of the payload
    /// area contain the forwarding pointer.
    ///
    /// # Safety
    /// - The allocation must be large enough to hold a pointer in the payload.
    /// - Must only be called during a scavenge while the from-space is not
    ///   being read by user code.
    #[inline]
    pub unsafe fn set_forwarding_address(&mut self, new_addr: *mut u8) {
        self.flags |= FLAG_FORWARDED;
        // SAFETY: The payload area is at least pointer-sized for any real object.
        unsafe {
            let fwd_slot = (self as *mut GcHeader).add(1) as *mut *mut u8;
            std::ptr::write(fwd_slot, new_addr);
        }
    }

    /// Read the forwarding address.
    ///
    /// # Safety
    /// Must only be called after `set_forwarding_address` and while the object
    /// is still valid.
    #[inline]
    pub unsafe fn forwarding_address(&self) -> *mut u8 {
        debug_assert!(self.is_forwarded());
        // SAFETY: set_forwarding_address wrote a valid pointer here.
        unsafe {
            let fwd_slot = (self as *const GcHeader).add(1) as *const *mut u8;
            std::ptr::read(fwd_slot)
        }
    }
}

impl fmt::Debug for GcHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcHeader")
            .field("size", &self.size)
            .field("type_tag", &self.tag())
            .field("forwarded", &self.is_forwarded())
            .field("marked", &self.is_marked())
            .field("pinned", &self.is_pinned())
            .field("tenured", &self.is_tenured())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(HEADER_SIZE, 8, "GcHeader must be exactly 8 bytes");
    }

    #[test]
    fn test_header_alignment() {
        assert_eq!(std::mem::align_of::<GcHeader>(), 4);
    }

    #[test]
    fn test_header_new() {
        let header = GcHeader::new(64, TypeTag::Object);
        assert_eq!(header.total_size(), 64);
        assert_eq!(header.payload_size(), 56);
        assert_eq!(header.tag(), TypeTag::Object);
        assert!(!header.is_forwarded());
        assert!(!header.is_marked());
        assert!(!header.is_pinned());
    }

    #[test]
    fn test_header_flags() {
        let mut header = GcHeader::new(64, TypeTag::String);

        header.set_marked();
        assert!(header.is_marked());

        header.clear_marked();
        assert!(!header.is_marked());

        header.set_pinned();
        assert!(header.is_pinned());

        header.set_tenured();
        assert!(header.is_tenured());
    }
}
