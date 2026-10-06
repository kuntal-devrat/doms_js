//! Inline cache (IC) infrastructure for fast property access.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcState {
    Uninitialized,
    Monomorphic,
    Polymorphic,
    Megamorphic,
}

#[derive(Debug, Clone)]
pub struct InlineCacheSlot {
    pub state: IcState,
    pub cached_shape_id: Option<u32>,
    pub cached_offset: Option<u32>,
}

impl InlineCacheSlot {
    pub const fn new() -> Self {
        Self { state: IcState::Uninitialized, cached_shape_id: None, cached_offset: None }
    }

    pub fn record_hit(&mut self, shape_id: u32, offset: u32) {
        match self.state {
            IcState::Uninitialized => {
                self.state = IcState::Monomorphic;
                self.cached_shape_id = Some(shape_id);
                self.cached_offset = Some(offset);
            }
            IcState::Monomorphic => {
                if self.cached_shape_id != Some(shape_id) {
                    self.state = IcState::Polymorphic;
                }
            }
            IcState::Polymorphic => {
                self.state = IcState::Megamorphic;
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
