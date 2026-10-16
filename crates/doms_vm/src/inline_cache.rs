//! Inline cache (IC) infrastructure for fast, polymorphic property access.
//!
//! Implements a 4-entry polymorphic inline cache following standard V8/JSC
//! engine design:
//! - [`IcState::Uninitialized`]: No shapes seen yet.
//! - [`IcState::Monomorphic`]: Exactly 1 shape seen. Fast single-comparison hit.
//! - [`IcState::Polymorphic`]: 2 to 4 distinct shapes seen. Unrolled linear search.
//! - [`IcState::Megamorphic`]: >4 shapes seen. Falls back to shape/prototype lookup.

pub const MAX_POLYMORPHIC_ENTRIES: usize = 4;

/// A cached shape and corresponding property slot offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShapeOffsetPair {
    pub shape_id: u32,
    pub offset: u32,
}

/// The state of an inline cache slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IcState {
    #[default]
    Uninitialized,
    Monomorphic,
    Polymorphic,
    Megamorphic,
}

/// A polymorphic inline cache slot holding up to 4 shape transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlineCacheSlot {
    pub state: IcState,
    pub name_idx: u16,
    pub entries: [ShapeOffsetPair; MAX_POLYMORPHIC_ENTRIES],
    pub entry_count: u8,
}

impl InlineCacheSlot {
    pub const fn new() -> Self {
        Self {
            state: IcState::Uninitialized,
            name_idx: 0,
            entries: [ShapeOffsetPair { shape_id: 0, offset: 0 }; MAX_POLYMORPHIC_ENTRIES],
            entry_count: 0,
        }
    }

    /// Fast-path lookup for a shape's property slot offset.
    #[inline]
    pub fn lookup(&self, name_idx: u16, shape_id: u32) -> Option<u32> {
        if self.state == IcState::Uninitialized || self.name_idx != name_idx {
            return None;
        }
        match self.state {
            IcState::Monomorphic => {
                if self.entries[0].shape_id == shape_id {
                    Some(self.entries[0].offset)
                } else {
                    None
                }
            }
            IcState::Polymorphic => {
                for i in 0..(self.entry_count as usize) {
                    if self.entries[i].shape_id == shape_id {
                        return Some(self.entries[i].offset);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Record a property access hit for a shape, updating state accordingly.
    pub fn record_hit(&mut self, name_idx: u16, shape_id: u32, offset: u32) {
        if self.state != IcState::Uninitialized && self.name_idx != name_idx {
            self.state = IcState::Uninitialized;
            self.entry_count = 0;
        }
        self.name_idx = name_idx;
        match self.state {
            IcState::Uninitialized => {
                self.state = IcState::Monomorphic;
                self.entries[0] = ShapeOffsetPair { shape_id, offset };
                self.entry_count = 1;
            }
            IcState::Monomorphic => {
                if self.entries[0].shape_id == shape_id {
                    self.entries[0].offset = offset;
                } else {
                    self.state = IcState::Polymorphic;
                    self.entries[1] = ShapeOffsetPair { shape_id, offset };
                    self.entry_count = 2;
                }
            }
            IcState::Polymorphic => {
                for i in 0..(self.entry_count as usize) {
                    if self.entries[i].shape_id == shape_id {
                        self.entries[i].offset = offset;
                        return;
                    }
                }
                if (self.entry_count as usize) < MAX_POLYMORPHIC_ENTRIES {
                    self.entries[self.entry_count as usize] = ShapeOffsetPair { shape_id, offset };
                    self.entry_count += 1;
                } else {
                    self.state = IcState::Megamorphic;
                }
            }
            IcState::Megamorphic => {}
        }
    }
}

impl Default for InlineCacheSlot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ic_uninitialized_to_monomorphic() {
        let mut ic = InlineCacheSlot::new();
        assert_eq!(ic.state, IcState::Uninitialized);
        assert_eq!(ic.lookup(1, 1), None);

        ic.record_hit(1, 1, 0);
        assert_eq!(ic.state, IcState::Monomorphic);
        assert_eq!(ic.lookup(1, 1), Some(0));
        assert_eq!(ic.lookup(1, 2), None);
        assert_eq!(ic.lookup(2, 1), None); // different name_idx
    }

    #[test]
    fn test_ic_polymorphic_up_to_four_shapes() {
        let mut ic = InlineCacheSlot::new();
        ic.record_hit(1, 10, 0);
        assert_eq!(ic.state, IcState::Monomorphic);

        ic.record_hit(1, 20, 1);
        assert_eq!(ic.state, IcState::Polymorphic);
        assert_eq!(ic.entry_count, 2);
        assert_eq!(ic.lookup(1, 10), Some(0));
        assert_eq!(ic.lookup(1, 20), Some(1));
        assert_eq!(ic.lookup(1, 30), None);

        ic.record_hit(1, 30, 2);
        assert_eq!(ic.state, IcState::Polymorphic);
        assert_eq!(ic.entry_count, 3);
        assert_eq!(ic.lookup(1, 30), Some(2));

        ic.record_hit(1, 40, 3);
        assert_eq!(ic.state, IcState::Polymorphic);
        assert_eq!(ic.entry_count, 4);
        assert_eq!(ic.lookup(1, 40), Some(3));
        assert_eq!(ic.lookup(1, 10), Some(0));
    }

    #[test]
    fn test_ic_megamorphic_transition() {
        let mut ic = InlineCacheSlot::new();
        ic.record_hit(1, 1, 0);
        ic.record_hit(1, 2, 1);
        ic.record_hit(1, 3, 2);
        ic.record_hit(1, 4, 3);
        assert_eq!(ic.state, IcState::Polymorphic);

        // 5th shape triggers Megamorphic
        ic.record_hit(1, 5, 4);
        assert_eq!(ic.state, IcState::Megamorphic);
        assert_eq!(ic.lookup(1, 1), None);
        assert_eq!(ic.lookup(1, 5), None);
    }
}
