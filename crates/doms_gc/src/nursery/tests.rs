//! Unit tests for the nursery.

use super::*;

const TEST_SPACE_SIZE: usize = 4096; // 4 KB for tests

#[test]
fn test_nursery_creation() {
    let nursery = Nursery::new(TEST_SPACE_SIZE);
    assert_eq!(nursery.space_size(), TEST_SPACE_SIZE);
    assert_eq!(nursery.used_bytes(), 0);
    assert_eq!(nursery.remaining_bytes(), TEST_SPACE_SIZE);
    assert_eq!(nursery.object_count(), 0);
}

#[test]
fn test_bump_allocation() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    // Allocate a 24-byte payload.
    let ptr = nursery.allocate(24, TypeTag::Object);
    assert!(ptr.is_some());

    let ptr = ptr.unwrap();
    assert!(!ptr.is_null());
    assert!(nursery.contains(ptr));

    // Used bytes should be header (8) + payload (24) = 32.
    assert_eq!(nursery.used_bytes(), 32);
    assert_eq!(nursery.object_count(), 1);
}

#[test]
fn test_multiple_allocations() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    let p1 = nursery.allocate(16, TypeTag::Object).unwrap();
    let p2 = nursery.allocate(32, TypeTag::String).unwrap();
    let p3 = nursery.allocate(8, TypeTag::Array).unwrap();

    // All pointers should be different and in from-space.
    assert_ne!(p1, p2);
    assert_ne!(p2, p3);
    assert!(nursery.contains(p1));
    assert!(nursery.contains(p2));
    assert!(nursery.contains(p3));
    assert_eq!(nursery.object_count(), 3);

    // Verify headers.
    unsafe {
        let h1 = &*((p1 as *const GcHeader).sub(1));
        assert_eq!(h1.tag(), TypeTag::Object);

        let h2 = &*((p2 as *const GcHeader).sub(1));
        assert_eq!(h2.tag(), TypeTag::String);
    }
}

#[test]
fn test_allocation_fills_nursery() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    // Each allocation: 8 (header) + 8 (payload, aligned) = 16 bytes.
    // 4096 / 16 = 256 allocations should fit.
    let mut count = 0;
    while nursery.allocate(8, TypeTag::Raw).is_some() {
        count += 1;
    }
    assert_eq!(count, 256);

    // Next allocation should fail.
    assert!(nursery.allocate(8, TypeTag::Raw).is_none());
}

#[test]
fn test_allocate_value() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    let ptr = unsafe { nursery.allocate_value(42u64, TypeTag::Raw) };
    assert!(ptr.is_some());

    let ptr = ptr.unwrap();
    assert_eq!(unsafe { *ptr }, 42u64);
}

#[test]
fn test_scavenge_with_roots() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    // Allocate 3 objects.
    let p1 = nursery.allocate(16, TypeTag::Object).unwrap();
    let _p2 = nursery.allocate(16, TypeTag::String).unwrap(); // Not rooted — should die.
    let p3 = nursery.allocate(16, TypeTag::Array).unwrap();

    // Write some data so we can verify it survives.
    unsafe {
        *(p1 as *mut u64) = 0xDEAD_BEEF;
        *(p3 as *mut u64) = 0xCAFE_BABE;
    }

    // Only root p1 and p3.
    let mut roots = vec![p1, p3];

    let survived = nursery.scavenge(&mut roots);

    // Two objects survived, each 24 bytes (8 header + 16 payload).
    assert_eq!(survived, 48);
    assert_eq!(nursery.object_count(), 2);

    // In our non-moving GC, roots remain at their exact memory locations!
    let new_p1 = roots[0];
    let new_p3 = roots[1];

    assert!(nursery.contains(new_p1));
    assert!(nursery.contains(new_p3));
    assert_eq!(new_p1, p1, "Non-moving GC preserves object addresses");
    assert_eq!(new_p3, p3, "Non-moving GC preserves object addresses");

    // Data should be preserved.
    unsafe {
        assert_eq!(*(new_p1 as *const u64), 0xDEAD_BEEF);
        assert_eq!(*(new_p3 as *const u64), 0xCAFE_BABE);
    }
}

#[test]
fn test_scavenge_forwarding() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    let p1 = nursery.allocate(16, TypeTag::Object).unwrap();

    let mut roots = vec![p1, p1];
    nursery.scavenge(&mut roots);

    assert_eq!(roots[0], p1);
    assert_eq!(roots[1], p1);
}

#[test]
fn test_scavenge_empty_roots() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    // Allocate some objects.
    nursery.allocate(16, TypeTag::Object).unwrap();
    nursery.allocate(16, TypeTag::String).unwrap();

    // No roots — everything should be collected.
    let mut roots: Vec<*mut u8> = vec![];
    let survived = nursery.scavenge(&mut roots);

    assert_eq!(survived, 0);
    assert_eq!(nursery.object_count(), 0);
}

#[test]
fn test_reset() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    nursery.allocate(16, TypeTag::Object).unwrap();
    nursery.allocate(16, TypeTag::String).unwrap();
    assert_eq!(nursery.object_count(), 2);

    nursery.reset();
    assert_eq!(nursery.object_count(), 0);
    assert_eq!(nursery.used_bytes(), 0);
}

#[test]
fn test_utilization() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    assert_eq!(nursery.utilization(), 0.0);

    // Fill half the nursery.
    let alloc_size = TEST_SPACE_SIZE / 2;
    nursery.allocate(alloc_size - HEADER_SIZE, TypeTag::Raw).unwrap();

    let util = nursery.utilization();
    assert!((util - 0.5).abs() < 0.01);
}

#[test]
fn test_contains() {
    let nursery = Nursery::new(TEST_SPACE_SIZE);
    let outside: u8 = 0;
    assert!(!nursery.contains(&outside as *const u8));
}

#[test]
fn test_scavenge_with_interior_references() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    // Register a tracer for TypeTag::Object that treats the first 8 bytes as a pointer slot.
    unsafe fn trace_first_slot(payload: *mut u8, visitor: &mut dyn FnMut(*mut *mut u8)) {
        let slot = payload as *mut *mut u8;
        visitor(slot);
    }
    nursery.register_tracer(TypeTag::Object, trace_first_slot);

    // Allocate child object B.
    let child_ptr = nursery.allocate(16, TypeTag::Raw).unwrap();
    unsafe {
        *(child_ptr as *mut u64) = 0xCAFEBABE;
    }

    // Allocate parent object A, containing a pointer to child B.
    let parent_ptr = nursery.allocate(16, TypeTag::Object).unwrap();
    unsafe {
        *(parent_ptr as *mut *mut u8) = child_ptr;
    }

    // Root ONLY parent A. Child B is NOT directly rooted.
    let mut roots = vec![parent_ptr];
    nursery.scavenge(&mut roots);

    let new_parent_ptr = roots[0];
    assert!(nursery.contains(new_parent_ptr));
    assert_eq!(new_parent_ptr, parent_ptr, "Parent address is preserved");

    // Verify child was also preserved via interior tracing
    let new_child_ptr = unsafe { *(new_parent_ptr as *const *mut u8) };
    assert!(nursery.contains(new_child_ptr));
    assert_eq!(new_child_ptr, child_ptr, "Child address is preserved");

    // Verify child payload is intact
    let child_val = unsafe { *(new_child_ptr as *const u64) };
    assert_eq!(child_val, 0xCAFEBABE);
}

#[test]
fn test_repeated_sweep_after_free_chunk_split_preserves_later_live_objects() {
    let mut nursery = Nursery::new(TEST_SPACE_SIZE);

    // A, B, C: A dies first, C stays live throughout.
    let a = nursery.allocate(96, TypeTag::Object).unwrap();
    let b = nursery.allocate(96, TypeTag::Object).unwrap();
    let c = nursery.allocate(96, TypeTag::Object).unwrap();

    // First sweep: only B and C are live, so A becomes a free chunk.
    let mut roots = vec![b, c];
    nursery.scavenge(&mut roots);

    // A smaller object splits A's free chunk. The remainder must keep a valid
    // free header; otherwise the next sweep stops scanning at the gap.
    let d = nursery.allocate(32, TypeTag::Object).unwrap();
    assert_eq!(d, a);

    // Second sweep: only C is live. If the scan stops at the split remainder,
    // the nursery is wrongly considered empty, reset, and C gets overwritten.
    let mut roots = vec![c];
    nursery.scavenge(&mut roots);
    assert!(nursery.bytes_allocated > 0, "live object C must keep the nursery from resetting");

    // Allocate until the nursery is full: no allocation may reuse C's address.
    unsafe {
        *(c as *mut u64) = 0x1122_3344_5566_7788;
    }
    while let Some(p) = nursery.allocate(96, TypeTag::Object) {
        assert_ne!(p, c, "allocation reused a live object's address");
    }
    assert_eq!(unsafe { *(c as *const u64) }, 0x1122_3344_5566_7788);
}
