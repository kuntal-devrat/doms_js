//! Object tracking and tracing garbage collection entry points.

use super::*;
use crate::object::VisitMode;
use crate::value::{PAYLOAD_MASK, TAG_MASK, TAG_OBJECT, TAG_STRING, TAG_SYMBOL};
use std::cell::Cell;
use std::collections::HashSet;

thread_local! {
    /// Outermost (highest address) native stack pointer observed on this thread
    /// while executing engine code.
    ///
    /// [`Vm::gc_collect`] conservatively scans the native stack between the
    /// current stack pointer and this boundary. That keeps `JsValue`s which are
    /// only held in Rust locals or registers at the moment a collection is
    /// triggered (allocation-triggered collections, builtins, JIT helpers) from
    /// being reclaimed. The boundary only ever moves outwards, and candidate
    /// words must exactly match a tracked object start before they are treated
    /// as references, so stale words can never corrupt unrelated memory.
    static STACK_BASE: Cell<usize> = const { Cell::new(0) };
}

/// Record the current native stack position as a conservative root-scan boundary.
///
/// This is a fallback for platforms where the thread stack bounds cannot be
/// queried: the conservative scan then only covers the region between the
/// scan's stack pointer and this boundary. On Linux/macOS the scan instead
/// walks up to the real top of the thread stack, so calling this is optional.
#[inline]
pub fn register_stack_base() {
    let probe: usize = 0;
    let sp = &probe as *const usize as usize;
    STACK_BASE.with(|base| {
        if sp > base.get() {
            base.set(sp);
        }
    });
}

thread_local! {
    /// Top (highest address) of this thread's native stack, detected once.
    static STACK_TOP: Cell<usize> = const { Cell::new(0) };
}

/// Query the operating system for the top of the current thread's stack.
///
/// Returns `0` when the query is unavailable (the caller then falls back to
/// the [`STACK_BASE`] boundary).
#[cfg(target_os = "linux")]
fn detect_native_stack_top() -> usize {
    unsafe {
        let mut attr = std::mem::MaybeUninit::<libc::pthread_attr_t>::zeroed();
        if libc::pthread_getattr_np(libc::pthread_self(), attr.as_mut_ptr()) != 0 {
            return 0;
        }
        let mut attr = attr.assume_init();
        let mut addr: *mut libc::c_void = std::ptr::null_mut();
        let mut size: libc::size_t = 0;
        let ok = libc::pthread_attr_getstack(&attr, &mut addr, &mut size) == 0;
        libc::pthread_attr_destroy(&mut attr);
        if ok && !addr.is_null() && size > 0 {
            addr as usize + size
        } else {
            0
        }
    }
}

#[cfg(target_os = "macos")]
fn detect_native_stack_top() -> usize {
    // `pthread_get_stackaddr_np` returns the stack base (highest address).
    unsafe { libc::pthread_get_stackaddr_np(libc::pthread_self()) as usize }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn detect_native_stack_top() -> usize {
    0
}

/// Mark a raw machine word if it encodes a reference to a tracked object.
///
/// Two encodings occur in registers and Rust locals:
/// - a raw `*mut JsObject` (exact payload start), and
/// - a NaN-boxed [`JsValue`] whose tag selects the object/string/symbol pointer
///   stored in its 48-bit payload.
#[inline]
fn gc_mark_word(
    word: usize,
    heap: &Heap,
    starts: &HashSet<usize>,
    live: &mut HashSet<usize>,
    worklist: &mut Vec<*mut JsObject>,
) {
    if word == 0 {
        return;
    }
    Vm::gc_mark_ptr(word as *mut JsObject, heap, starts, live, worklist);
    let bits = word as u64;
    if matches!(bits & TAG_MASK, TAG_OBJECT | TAG_STRING | TAG_SYMBOL) {
        let payload = (bits & PAYLOAD_MASK) as usize;
        if payload != 0 {
            Vm::gc_mark_ptr(payload as *mut JsObject, heap, starts, live, worklist);
        }
    }
}

impl Vm {
    // Allocation helpers

    #[inline]
    pub fn track_ptr(&mut self, ptr: *mut JsObject) {
        self.objects.push(ptr);
        if let Some(max) = self.max_objects {
            if self.objects.len() >= max {
                self.oom = true;
            }
        }
    }

    /// Mark a tracked object as live.
    ///
    /// Returns `true` when the pointer was newly marked. Only exact payload
    /// starts of tracked objects are accepted, which makes it impossible for a
    /// stale or interior pointer to corrupt memory via a mark-bit write.
    #[inline]
    fn gc_mark_ptr(
        ptr: *mut JsObject,
        heap: &Heap,
        starts: &HashSet<usize>,
        live: &mut HashSet<usize>,
        worklist: &mut Vec<*mut JsObject>,
    ) -> bool {
        let key = ptr as usize;
        if !starts.contains(&key) || !live.insert(key) {
            return false;
        }
        if heap.contains(key as *const u8) {
            // All heap spaces place a `GcHeader` directly before the payload.
            unsafe {
                let header = &mut *((ptr as *mut doms_gc::GcHeader).sub(1));
                header.set_marked();
            }
        }
        worklist.push(ptr);
        true
    }

    /// Mark a value if it refers to a tracked heap object.
    #[inline]
    fn gc_mark_value(
        val: JsValue,
        heap: &Heap,
        starts: &HashSet<usize>,
        live: &mut HashSet<usize>,
        worklist: &mut Vec<*mut JsObject>,
    ) -> bool {
        match val.as_heap_ptr() {
            Some(p) => Self::gc_mark_ptr(p as *mut JsObject, heap, starts, live, worklist),
            None => false,
        }
    }

    /// Drain the mark worklist, tracing references according to `mode`.
    fn gc_drain_worklist(
        heap: &Heap,
        starts: &HashSet<usize>,
        live: &mut HashSet<usize>,
        worklist: &mut Vec<*mut JsObject>,
        mode: VisitMode,
    ) {
        while let Some(ptr) = worklist.pop() {
            let obj = unsafe { &mut *ptr };
            obj.visit_values_mut_mode(mode, |val| {
                Self::gc_mark_value(*val, heap, starts, live, worklist);
            });
        }
    }

    /// Whether a reference is safe to keep: primitives always are, heap values
    /// only while their target object is marked live.
    #[inline]
    fn gc_value_is_live(val: JsValue, live: &HashSet<usize>) -> bool {
        match val.as_heap_ptr() {
            Some(p) => live.contains(&(p as usize)),
            None => true,
        }
    }

    /// Conservatively scan the native stack and registers for pointers to
    /// tracked objects.
    ///
    /// The scan reads the region between the current stack pointer and the top
    /// of the thread's stack, so locals of *every* frame on the current call
    /// chain are visible — including embedder frames above the VM entry points.
    /// Both raw object pointers and NaN-boxed `JsValue`s are recognised, and
    /// (on Linux) callee-saved registers are spilled and scanned as well.
    ///
    /// Only exact object starts are accepted, so a scanned word can never be
    /// mistaken for an interior pointer.
    ///
    /// The scan deliberately reads the entire live stack region (including
    /// caller frames), which is why ASan builds disable stack instrumentation
    /// for this function only.
    #[cfg_attr(dom_asan, sanitize(address = "off"))]
    fn gc_scan_native_stack(
        heap: &Heap,
        starts: &HashSet<usize>,
        live: &mut HashSet<usize>,
        worklist: &mut Vec<*mut JsObject>,
    ) {
        let probe: usize = 0;
        let sp = &probe as *const usize as usize;
        // Prefer the true top of the thread stack; fall back to the boundary
        // recorded by `register_stack_base` where the OS query is unavailable.
        let recorded = STACK_BASE.with(|b| b.get());
        let top = STACK_TOP.with(|cell| {
            let cached = cell.get();
            if cached != 0 {
                return cached;
            }
            let detected = detect_native_stack_top();
            cell.set(detected);
            detected
        });
        let base = if top > recorded { top } else { recorded };
        if base == 0 || base <= sp {
            return;
        }
        let word_size = std::mem::size_of::<usize>();
        let align = std::mem::align_of::<usize>();
        let (lo, hi) = heap.address_bounds();
        let mut addr = (sp + align - 1) & !(align - 1);
        // The stack grows downwards, so [sp, base] covers the current call
        // chain and every frame above it. Every page in this range has been
        // mapped: the current chain spans it contiguously.
        while addr + word_size <= base {
            let word = unsafe { *(addr as *const usize) };
            // Cheap address-range filter before the exact object-table probe;
            // the vast majority of stack words are not heap references.
            if word >= lo && word < hi {
                Vm::gc_mark_ptr(word as *mut JsObject, heap, starts, live, worklist);
            } else {
                let bits = word as u64;
                if matches!(bits & TAG_MASK, TAG_OBJECT | TAG_STRING | TAG_SYMBOL) {
                    let payload = (bits & PAYLOAD_MASK) as usize;
                    if payload >= lo && payload < hi {
                        Vm::gc_mark_ptr(payload as *mut JsObject, heap, starts, live, worklist);
                    }
                }
            }
            addr += align;
        }
        Self::gc_scan_registers(heap, starts, live, worklist);
    }

    /// Spill callee-saved registers and scan them like stack words.
    ///
    /// Values held in registers across a collection-triggering call must live
    /// in callee-saved registers, which is exactly the set captured here.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[cfg_attr(dom_asan, sanitize(address = "off"))]
    fn gc_scan_registers(
        heap: &Heap,
        starts: &HashSet<usize>,
        live: &mut HashSet<usize>,
        worklist: &mut Vec<*mut JsObject>,
    ) {
        let mut ctx = std::mem::MaybeUninit::<libc::ucontext_t>::uninit();
        // SAFETY: `getcontext` only writes the provided context.
        if unsafe { libc::getcontext(ctx.as_mut_ptr()) } != 0 {
            return;
        }
        let ctx = unsafe { ctx.assume_init_ref() };
        for reg in [
            libc::REG_RBX,
            libc::REG_RBP,
            libc::REG_R12,
            libc::REG_R13,
            libc::REG_R14,
            libc::REG_R15,
        ] {
            let word = ctx.uc_mcontext.gregs[reg as usize] as usize;
            gc_mark_word(word, heap, starts, live, worklist);
        }
    }

    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    fn gc_scan_registers(
        _heap: &Heap,
        _starts: &HashSet<usize>,
        _live: &mut HashSet<usize>,
        _worklist: &mut Vec<*mut JsObject>,
    ) {
    }

    /// Perform a full tracing garbage collection cycle across all live roots.
    ///
    /// Collection is a non-moving in-place mark and sweep. Weak references are
    /// handled with ephemeron semantics: a `WeakMap` value is only kept alive
    /// while its key is, and dead weak entries are pruned so the VM can never
    /// observe dangling references.
    pub fn gc_collect(&mut self) -> usize {
        if self.gc_suppressed {
            // See `Vm::gc_suppressed`: collection during builtin construction
            // would free objects that registration code is still using.
            return 0;
        }
        let mut live: HashSet<usize> = HashSet::new();
        let mut worklist: Vec<*mut JsObject> = Vec::new();

        // The old generation is only swept lazily (when it fills up), so mark
        // bits there can survive from an earlier cycle. Clear them before
        // re-marking. The nursery and large-object spaces are swept on every
        // cycle, which already resets their marks.
        for &ptr in &self.objects {
            if self.heap.old_gen.contains(ptr as *const u8) {
                unsafe {
                    let header = &mut *((ptr as *mut doms_gc::GcHeader).sub(1));
                    header.clear_marked();
                }
            }
        }

        // Exact payload starts of every object the VM tracks. All heap values
        // in the VM point at tracked `JsObject` payloads, so this set validates
        // candidate pointers precisely (used for conservative scanning and for
        // guarding mark-bit writes).
        let starts: HashSet<usize> = self.objects.iter().map(|p| *p as usize).collect();

        {
            let heap = &self.heap;

            // Marking is expanded as a macro (rather than a closure) so the
            // same borrows of `live`/`worklist` can be reused at every root.
            macro_rules! mark {
                ($val:expr) => {
                    Self::gc_mark_value($val, heap, &starts, &mut live, &mut worklist)
                };
                (raw $ptr:expr) => {
                    Self::gc_mark_ptr($ptr, heap, &starts, &mut live, &mut worklist)
                };
            }

            // 1. Mark roots.

            // 1a. Intrinsic roots held directly by the VM.
            if let Some(pending) = self.pending_alloc {
                mark!(raw pending);
            }
            mark!(self.global_object);
            for proto in [
                self.object_proto,
                self.array_proto,
                self.function_proto,
                self.regexp_proto,
                self.string_proto,
                self.number_proto,
                self.boolean_proto,
                self.symbol_proto,
                self.promise_proto,
                self.date_proto,
                self.iterator_proto,
                self.array_iterator_proto,
                self.map_iterator_proto,
                self.set_iterator_proto,
                self.string_iterator_proto,
                self.generator_proto,
                self.wrap_for_valid_iterator_proto,
                self.iterator_helper_proto,
                self.current_new_target,
                self.symbol_iterator,
                self.symbol_async_iterator,
                self.symbol_to_string_tag,
                self.symbol_is_concat_spreadable,
                self.symbol_species,
                self.symbol_to_primitive,
                self.symbol_has_instance,
                self.symbol_match,
                self.symbol_match_all,
                self.symbol_replace,
                self.symbol_search,
                self.symbol_split,
                self.symbol_unscopables,
            ]
            .into_iter()
            .flatten()
            {
                mark!(proto);
            }
            for s in self.symbol_registry.values() {
                mark!(*s);
            }

            // 1b. Interpreter state.
            for val in &self.temp_roots {
                mark!(*val);
            }
            for val in &self.stack {
                mark!(*val);
            }
            for frame in &self.frames {
                if let Some(gv) = frame.gen_val {
                    mark!(gv);
                }
                if let Some(cv) = frame.callee_val {
                    mark!(cv);
                }
                mark!(frame.this_val);
                for local in &frame.locals {
                    mark!(*local);
                }
                for scope in &frame.scopes {
                    mark!(*scope);
                }
                if let Some(env) = frame.env_obj {
                    mark!(env);
                }
                if let Some(args_val) = frame.arguments {
                    mark!(args_val);
                }
                if let Some(ref raw_args) = frame.raw_args {
                    for arg in raw_args {
                        mark!(*arg);
                    }
                }
                if let Some(nt) = frame.new_target {
                    mark!(nt);
                }
                if let Some(ho) = frame.home_object {
                    mark!(ho);
                }
                if let Some(f) = &frame.callee_func {
                    for uv in &f.upvalues {
                        mark!(*uv);
                    }
                    if let Some(ho) = f.home_object {
                        mark!(ho);
                    }
                    if let Some(proto) = f.prototype {
                        mark!(proto);
                    }
                }
            }

            // 1c. Globals, modules and namespaces.
            for val in self.globals.values() {
                mark!(*val);
            }
            for val in self.global_lexicals.values() {
                mark!(*val);
            }
            for val in self.modules.values() {
                mark!(*val);
            }
            for val in self.current_exports.values() {
                mark!(*val);
            }
            for val in self.module_namespace_objects.iter() {
                mark!(*val);
            }
            if let Some(module_obj) = self.current_module_obj {
                mark!(module_obj);
            }
            for (module, _) in self.imported_bindings.values() {
                mark!(*module);
            }
            for val in self.star_exports.values() {
                mark!(*val);
            }
            for ((module, _), (value, _)) in self.module_reexports.iter() {
                mark!(*module);
                mark!(*value);
            }

            // 1d. Pending jobs.
            for timer in self.timers.values() {
                mark!(timer.callback);
                for arg in &timer.args {
                    mark!(*arg);
                }
            }
            for (callee, args) in &self.microtasks {
                mark!(*callee);
                for arg in args {
                    mark!(*arg);
                }
            }

            // 1e. Conservative native-stack scan. This keeps values that only
            //     exist in Rust locals/registers at the moment of collection.
            Self::gc_scan_native_stack(heap, &starts, &mut live, &mut worklist);
        }

        // 2. Trace transitive strongly-held references.
        Self::gc_drain_worklist(&self.heap, &starts, &mut live, &mut worklist, VisitMode::Strong);

        // 3. Ephemeron pass: `WeakMap` values are only kept alive while their
        //    key is alive. Values may reference other weak keys, so iterate to
        //    a fixpoint.
        loop {
            let mut newly_marked = false;
            for &ptr in &self.objects {
                if !live.contains(&(ptr as usize)) {
                    continue;
                }
                let obj = unsafe { &*ptr };
                if let ObjectKind::WeakMap(entries) = &obj.kind {
                    for (k, v) in entries {
                        if Self::gc_value_is_live(*k, &live)
                            && Self::gc_mark_value(*v, &self.heap, &starts, &mut live, &mut worklist)
                        {
                            newly_marked = true;
                        }
                    }
                }
            }
            if !newly_marked {
                break;
            }
            Self::gc_drain_worklist(&self.heap, &starts, &mut live, &mut worklist, VisitMode::Strong);
        }

        // 4. Clear dead weak references so the VM can never observe dangling
        //    values, and queue finalization cleanup jobs for dead targets.
        let mut cleanup_jobs: Vec<(JsValue, Vec<JsValue>)> = Vec::new();
        for &ptr in &self.objects {
            if !live.contains(&(ptr as usize)) {
                continue;
            }
            let obj = unsafe { &mut *ptr };
            match &mut obj.kind {
                ObjectKind::WeakRef(target) => {
                    if !Self::gc_value_is_live(*target, &live) {
                        *target = JsValue::undefined();
                    }
                }
                ObjectKind::WeakMap(entries) => {
                    entries.retain(|(k, _)| Self::gc_value_is_live(*k, &live));
                }
                ObjectKind::WeakSet(elements) => {
                    elements.retain(|el| Self::gc_value_is_live(*el, &live));
                }
                ObjectKind::FinalizationRegistry { callback, entries } => {
                    let callback = *callback;
                    entries.retain(|(target, held, _token)| {
                        if Self::gc_value_is_live(*target, &live) {
                            true
                        } else {
                            cleanup_jobs.push((callback, vec![*held]));
                            false
                        }
                    });
                }
                _ => {}
            }
        }
        for job in cleanup_jobs {
            self.microtasks.push_back(job);
        }

        // 5. Sweep dead objects.
        let before_count = self.objects.len();
        let mut i = 0;
        while i < self.objects.len() {
            let ptr = self.objects[i];
            if !live.contains(&(ptr as usize)) {
                unsafe {
                    std::ptr::drop_in_place(ptr);
                    if !self.heap.contains(ptr as *const u8) {
                        drop(Box::from_raw(ptr));
                    }
                }
                self.objects.swap_remove(i);
            } else {
                i += 1;
            }
        }

        self.heap.nursery.sweep_marked();
        // The large-object space is cheap to sweep (a bounded list of
        // allocations), so dead large objects are reclaimed every cycle. The
        // old generation is swept lazily from the allocation fallback instead:
        // scanning its full extent on every cycle would be costly.
        self.heap.large_objects.sweep();
        crate::jit::NURSERY_PTRS.with(|cell| {
            let ptrs = unsafe { &mut *cell.get() };
            if !ptrs.bump.is_null() {
                ptrs.bump = self.heap.nursery.cursor();
                ptrs.limit = self.heap.nursery.limit();
            }
        });
        crate::jit::clear_jit_call_cache();
        crate::jit::clear_jit_array_cache();
        before_count - self.objects.len()
    }

    /// Perform a generational minor GC (in-place non-moving mark and sweep).
    pub fn scavenge_nursery(&mut self) {
        self.gc_collect();
    }
}
