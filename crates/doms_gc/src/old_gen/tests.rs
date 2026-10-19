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
