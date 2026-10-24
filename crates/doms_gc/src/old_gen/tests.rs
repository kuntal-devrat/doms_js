//! Unit tests for the old generation.

use super::*;
use crate::header::TypeTag;
use crate::nursery::Nursery;

#[test]
fn test_old_gen_creation() {
    let old_gen = OldGen::new(4096);
    assert_eq!(old_gen.object_count, 0);
    assert_eq!(old_gen.bytes_used, 0);
}

#[test]
fn test_promote_from_nursery() {
    let mut nursery = Nursery::new(4096);
    let mut old_gen = OldGen::new(4096);

    // Allocate in nursery.
    let ptr = nursery.allocate(16, TypeTag::Object).unwrap();
    unsafe {
        *(ptr as *mut u64) = 0xDEAD_BEEF;
    }

    // Promote to old gen.
    let new_ptr = unsafe { old_gen.promote(ptr) };
    assert!(new_ptr.is_some());
    let new_ptr = new_ptr.unwrap();

    assert!(old_gen.contains(new_ptr));
    assert_eq!(old_gen.object_count, 1);

    // Data should be preserved.
    unsafe {
        assert_eq!(*(new_ptr as *const u64), 0xDEAD_BEEF);
    }
}

#[test]
fn test_incremental_marking() {
    let mut nursery = Nursery::new(4096);
    let mut old_gen = OldGen::new(4096);

    // Allocate two objects and promote them
    let p1 = nursery.allocate(16, TypeTag::Object).unwrap();
    let p2 = nursery.allocate(16, TypeTag::Object).unwrap();

    let old_p1 = unsafe { old_gen.promote(p1).unwrap() };
    let old_p2 = unsafe { old_gen.promote(p2).unwrap() };

    assert!(!old_gen.is_marked(old_p1));
    assert!(!old_gen.is_marked(old_p2));
    assert_eq!(old_gen.mark_state, MarkState::Idle);

    // Start incremental marking with old_p1 and old_p2 as roots
    let roots = vec![old_p1, old_p2];
    old_gen.start_incremental_marking(&roots);

    assert!(old_gen.is_marked(old_p1));
    assert!(old_gen.is_marked(old_p2));
    assert_eq!(old_gen.mark_state, MarkState::MarkingObjects);
    assert_eq!(old_gen.mark_worklist.len(), 2);

    // Step 1: work limit 1 -> should process 1 object, not finished yet
    let finished = old_gen.mark_step(1);
    assert!(!finished);
    assert_eq!(old_gen.mark_state, MarkState::MarkingObjects);
    assert_eq!(old_gen.mark_worklist.len(), 1);

    // Step 2: work limit 1 -> should process the remaining object and finish
    let finished2 = old_gen.mark_step(1);
    assert!(finished2);
    assert_eq!(old_gen.mark_state, MarkState::Complete);
    assert!(old_gen.mark_worklist.is_empty());

    // Reset marks
    old_gen.reset_marks();
    assert_eq!(old_gen.mark_state, MarkState::Idle);
    assert!(!old_gen.is_marked(old_p1));
    assert!(!old_gen.is_marked(old_p2));
}

#[test]
fn test_old_gen_compaction() {
    let mut nursery = Nursery::new(4096);
    let mut old_gen = OldGen::new(4096);

    let p1 = nursery.allocate(16, TypeTag::Object).unwrap();
    let p2 = nursery.allocate(16, TypeTag::Object).unwrap();

    let old_p1 = unsafe { old_gen.promote(p1).unwrap() };
    let _old_p2 = unsafe { old_gen.promote(p2).unwrap() };

    assert_eq!(old_gen.object_count, 2);

    // Mark only p1
    old_gen.start_incremental_marking(&[old_p1]);
    assert!(old_gen.is_marked(old_p1));

    // Compact: should collect unmarked p2 and retain p1
    let reclaimed = old_gen.compact();
    assert!(reclaimed > 0);
    assert_eq!(old_gen.object_count, 1);
}

#[test]
fn test_old_gen_sweep_and_reuse() {
    let mut nursery = Nursery::new(4096);
    let mut old_gen = OldGen::new(4096);

    let p1 = nursery.allocate(16, TypeTag::Object).unwrap();
    let p2 = nursery.allocate(16, TypeTag::Object).unwrap();
    let p3 = nursery.allocate(16, TypeTag::Object).unwrap();

    let old_p1 = unsafe { old_gen.promote(p1).unwrap() };
    let _old_p2 = unsafe { old_gen.promote(p2).unwrap() };
    let old_p3 = unsafe { old_gen.promote(p3).unwrap() };

    assert_eq!(old_gen.object_count, 3);

    // Mark only p1 and p3 (p2 is dead)
    old_gen.start_incremental_marking(&[old_p1, old_p3]);

    // Sweep: p2 is swept without moving p1 or p3
    let reclaimed = old_gen.sweep();
    assert!(reclaimed > 0);
    assert_eq!(old_gen.object_count, 2);
    assert!(!old_gen.free_chunks.is_empty());

    // Live object pointers must NOT have changed!
    assert!(old_gen.contains(old_p1));
    assert!(old_gen.contains(old_p3));

    // Allocating another object should reuse the freed chunk
    let p4 = nursery.allocate(16, TypeTag::Object).unwrap();
    let old_p4 = unsafe { old_gen.promote(p4).unwrap() };
    assert_eq!(old_gen.object_count, 3);
    assert!(old_gen.contains(old_p4));
}

#[test]
fn test_old_gen_repeated_sweep_scans_past_split_remainder() {
    let mut nursery = Nursery::new(4096);
    let mut old_gen = OldGen::new(4096);

    let p1 = nursery.allocate(16, TypeTag::Object).unwrap();
    let p2 = nursery.allocate(96, TypeTag::Object).unwrap();
    let p3 = nursery.allocate(16, TypeTag::Object).unwrap();
    let old_p1 = unsafe { old_gen.promote(p1).unwrap() };
    let _old_p2 = unsafe { old_gen.promote(p2).unwrap() };
    let old_p3 = unsafe { old_gen.promote(p3).unwrap() };

    // First sweep frees p2, leaving a large free chunk between p1 and p3.
    old_gen.start_incremental_marking(&[old_p1, old_p3]);
    let reclaimed = old_gen.sweep();
    assert!(reclaimed > 0);
    assert_eq!(old_gen.object_count, 2);

    // Allocate a smaller block from p2's chunk: it splits, and the remainder
    // must keep a valid free header so the next sweep can scan past it.
    let p4 = old_gen.allocate(8, TypeTag::Object).unwrap();
    assert_eq!(p4, _old_p2);

    // Second sweep: only p3 (which sits *after* the split remainder) is live.
    old_gen.start_incremental_marking(&[old_p3]);
    let reclaimed = old_gen.sweep();
    assert!(reclaimed > 0, "dead objects after a split remainder must be reclaimed");
    assert_eq!(old_gen.object_count, 1, "the live object after the remainder must survive");
    assert!(old_gen.bytes_used > 0);
    assert!(!old_gen.is_marked(old_p3), "live mark must be cleared by the sweep");
}

#[test]
fn test_old_gen_incremental_pointer_tracing() {
    let mut old_gen = OldGen::new(4096);

    // Register a tracer for TypeTag::Object where the first 8 bytes point to another object
    unsafe fn trace_one_slot(payload: *mut u8, visit: &mut dyn FnMut(*mut *mut u8)) {
        let slot = payload as *mut *mut u8;
        visit(slot);
    }
    old_gen.register_tracer(TypeTag::Object, trace_one_slot);

    // Allocate p1 (parent), p2 (child), p3 (unreachable garbage)
    let p1 = old_gen.allocate(16, TypeTag::Object).unwrap();
    let p2 = old_gen.allocate(16, TypeTag::Object).unwrap();
    let p3 = old_gen.allocate(16, TypeTag::Object).unwrap();

    // p1 points to p2
    unsafe {
        *(p1 as *mut *mut u8) = p2;
        *(p2 as *mut *mut u8) = std::ptr::null_mut();
        *(p3 as *mut *mut u8) = std::ptr::null_mut();
    }

    // Only p1 is in the roots!
    old_gen.start_incremental_marking(&[p1]);
    assert!(old_gen.is_marked(p1));
    assert!(!old_gen.is_marked(p2));
    assert!(!old_gen.is_marked(p3));
    assert_eq!(old_gen.mark_worklist.len(), 1);

    // Step 1: processes p1. Through the tracer, p2 must be discovered and marked!
    let finished1 = old_gen.mark_step(1);
    assert!(!finished1, "Marking should not be complete as p2 was discovered");
    assert!(old_gen.is_marked(p2), "p2 must be marked via interior pointer tracing");
    assert_eq!(old_gen.mark_worklist.len(), 1);

    // Step 2: processes p2.
    let finished2 = old_gen.mark_step(1);
    assert!(finished2, "Marking should now be complete");
    assert_eq!(old_gen.mark_state, MarkState::Complete);

    // Sweep: p1 and p2 must survive, p3 must be reclaimed!
    let reclaimed = old_gen.sweep();
    assert!(reclaimed > 0);
    assert_eq!(old_gen.object_count, 2);
    assert!(old_gen.contains(p1));
    assert!(old_gen.contains(p2));
}

#[test]
fn test_heap_concurrent_marking_and_write_barrier() {
    use crate::heap::{Heap, HeapConfig};

    let mut heap = Heap::new(HeapConfig { nursery_size: 4096, old_gen_size: 8192 });

    unsafe fn trace_first_slot(payload: *mut u8, visit: &mut dyn FnMut(*mut *mut u8)) {
        let slot = payload as *mut *mut u8;
        visit(slot);
    }
    heap.register_tracer(TypeTag::Object, trace_first_slot);

    // Allocate in old gen directly
    let root_obj = heap.old_gen.allocate(16, TypeTag::Object).unwrap();
    let child_obj = heap.old_gen.allocate(16, TypeTag::Object).unwrap();
    let barrier_obj = heap.old_gen.allocate(16, TypeTag::Object).unwrap();
    let garbage_obj = heap.old_gen.allocate(16, TypeTag::Object).unwrap();

    unsafe {
        *(root_obj as *mut *mut u8) = child_obj;
        *(child_obj as *mut *mut u8) = std::ptr::null_mut();
        *(barrier_obj as *mut *mut u8) = std::ptr::null_mut();
        *(garbage_obj as *mut *mut u8) = std::ptr::null_mut();
    }

    heap.add_root(root_obj);

    // Start concurrent marking
    heap.start_concurrent_marking();
    assert!(heap.is_concurrent_marking_active());

    // Main thread mutates child_obj to point to barrier_obj via write barrier
    unsafe {
        let slot = child_obj as *mut *mut u8;
        *slot = barrier_obj;
        heap.write_barrier(slot);
    }

    // Allow concurrent worker to run and finish
    for _ in 0..50 {
        if heap.is_concurrent_marking_done() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Finalize concurrent marking
    let completed = heap.finish_concurrent_marking();
    assert!(completed);
    assert!(!heap.is_concurrent_marking_active());

    // root_obj, child_obj, and barrier_obj (via write barrier) must be marked!
    assert!(heap.old_gen.is_marked(root_obj));
    assert!(heap.old_gen.is_marked(child_obj));
    assert!(heap.old_gen.is_marked(barrier_obj));
    assert!(!heap.old_gen.is_marked(garbage_obj));

    // Sweep: 3 objects survive, 1 garbage reclaimed
    let reclaimed = heap.old_gen.sweep();
    assert!(reclaimed > 0);
    assert_eq!(heap.old_gen.object_count, 3);
}
