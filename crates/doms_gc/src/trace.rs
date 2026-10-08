//! The `Trace` trait and `Tracer` — the GC's object graph walker.
//!
//! Every type that can be allocated on the GC heap must implement [`Trace`].
//! During a collection, the GC calls `trace()` on each root, which in turn
//! calls `tracer.trace_ref()` for each GC reference the object holds.
//!
//! This design means that adding a new built-in type to `doms_runtime` only
//! requires implementing `Trace` — you don't need to understand the GC internals.

use crate::gc_ref::GcRef;

/// A tracer that the GC passes to [`Trace::trace`] implementations.
///
/// The tracer is the GC's "callback" — when an object traces its fields,
/// it tells the tracer about each GC reference it holds. The tracer then
/// ensures those referenced objects are kept alive (and possibly moved).
pub struct Tracer {
    /// Internal callback that processes each traced reference.
    /// During nursery scavenge, this copies the object to to-space.
    /// During old-gen mark, this sets the mark bit.
    trace_fn: Box<dyn FnMut(*mut u8)>,
}

impl Tracer {
    /// Create a new tracer with the given callback.
    ///
    /// The callback receives a mutable pointer to the GcRef's internal pointer,
    /// allowing the GC to update it (e.g., during a moving collection).
    pub fn new(trace_fn: impl FnMut(*mut u8) + 'static) -> Self {
        Self { trace_fn: Box::new(trace_fn) }
    }

    /// Trace a single GC reference.
    ///
    /// This informs the GC that the object at `gc_ref` is reachable.
    /// During a moving collection, the `gc_ref`'s internal pointer may
    /// be updated to point to the object's new location.
    #[inline]
    pub fn trace_ref(&mut self, ptr: *mut u8) {
        (self.trace_fn)(ptr);
    }

    /// Trace a GcRef, updating it if the object was moved.
    ///
    /// This is the primary method that `Trace` implementations call.
    #[inline]
    pub fn trace_gc_ref<T>(&mut self, gc_ref: &GcRef<T>) {
        if !gc_ref.is_null() {
            self.trace_ref(gc_ref.as_raw());
        }
    }
}

/// Trait implemented by all types that can be stored on the GC heap.
///
/// # Implementing `Trace`
///
/// For types that contain no GC references (leaf types like strings or numbers),
/// implement `trace` as a no-op:
///
/// ```rust,ignore
/// impl Trace for MyLeafType {
///     fn trace(&self, _tracer: &mut Tracer) {
///         // No GC references to trace.
///     }
/// }
/// ```
///
/// For types that contain GC references, call `tracer.trace_gc_ref()` for each one:
///
/// ```rust,ignore
/// impl Trace for MyObject {
///     fn trace(&self, tracer: &mut Tracer) {
///         tracer.trace_gc_ref(&self.prototype);
///         for prop in &self.properties {
///             tracer.trace_gc_ref(&prop.value);
///         }
///     }
/// }
/// ```
///
/// # Safety
///
/// Implementors **must** trace all GC references they hold. Failing to trace
/// a reference will cause the GC to collect a live object, leading to
/// use-after-free.
pub trait Trace {
    /// Walk all GC references held by this object.
    fn trace(&self, tracer: &mut Tracer);

    /// Returns the number of GC references this object directly holds.
    /// Used for debugging and heap analysis. Optional — defaults to 0.
    fn reference_count_hint(&self) -> usize {
        0
    }
}

// ── Blanket implementations for common types ──

impl Trace for () {
    #[inline]
    fn trace(&self, _tracer: &mut Tracer) {}
}

impl Trace for bool {
    #[inline]
    fn trace(&self, _tracer: &mut Tracer) {}
}

impl Trace for i32 {
    #[inline]
    fn trace(&self, _tracer: &mut Tracer) {}
}

impl Trace for u32 {
    #[inline]
    fn trace(&self, _tracer: &mut Tracer) {}
}

impl Trace for f64 {
    #[inline]
    fn trace(&self, _tracer: &mut Tracer) {}
}

impl Trace for String {
    #[inline]
    fn trace(&self, _tracer: &mut Tracer) {}
}

impl<T: Trace> Trace for Vec<T> {
    fn trace(&self, tracer: &mut Tracer) {
        for item in self {
            item.trace(tracer);
        }
    }

    fn reference_count_hint(&self) -> usize {
        self.iter().map(|item| item.reference_count_hint()).sum()
    }
}

impl<T: Trace> Trace for Option<T> {
    fn trace(&self, tracer: &mut Tracer) {
        if let Some(inner) = self {
            inner.trace(tracer);
        }
    }
}

impl<T: Trace> Trace for Box<T> {
    fn trace(&self, tracer: &mut Tracer) {
        (**self).trace(tracer);
    }
}

use crate::header::TypeTag;

/// Function signature for walking interior reference slots of an object payload.
///
/// The first argument is the object payload pointer (`*mut u8`).
/// The second argument is a visitor callback called for each slot (`*mut *mut u8`)
/// holding a GC pointer within the payload.
pub type TraceVisitor = unsafe fn(*mut u8, &mut dyn FnMut(*mut *mut u8));

/// Registry mapping type tags to their interior trace visitor functions.
#[derive(Clone, Copy)]
pub struct TypeRegistry {
    visitors: [Option<TraceVisitor>; 32],
}

impl TypeRegistry {
    pub const fn new() -> Self {
        Self {
            visitors: [None; 32],
        }
    }

    /// Register a trace visitor function for a given TypeTag.
    pub fn register(&mut self, tag: TypeTag, visitor: TraceVisitor) {
        let idx = tag as usize;
        if idx < self.visitors.len() {
            self.visitors[idx] = Some(visitor);
        }
    }

    /// Trace the interior pointers of a payload with the given type tag.
    ///
    /// # Safety
    /// `payload` must point to a valid object payload for `tag`.
    #[inline]
    pub unsafe fn trace(&self, tag: TypeTag, payload: *mut u8, visit: &mut dyn FnMut(*mut *mut u8)) {
        let idx = tag as usize;
        if idx < self.visitors.len() {
            if let Some(v) = self.visitors[idx] {
                unsafe { v(payload, visit) };
            }
        }
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockObj {
        refs: Vec<*mut u8>,
    }

    impl Trace for MockObj {
        fn trace(&self, tracer: &mut Tracer) {
            for r in &self.refs {
                tracer.trace_ref(*r);
            }
        }

        fn reference_count_hint(&self) -> usize {
            self.refs.len()
        }
    }

    #[test]
    fn test_trace_counts() {
        let obj = MockObj { refs: vec![std::ptr::null_mut(); 3] };
        assert_eq!(obj.reference_count_hint(), 3);
    }

    #[test]
    fn test_tracer_calls_callback() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count_clone = std::sync::Arc::clone(&count);
        {
            let mut tracer = Tracer::new(move |_ptr| {
                count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
            tracer.trace_ref(std::ptr::null_mut());
            tracer.trace_ref(std::ptr::null_mut());
        }
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
