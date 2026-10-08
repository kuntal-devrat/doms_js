//! Hidden classes (Shapes) for fast property access.

use std::collections::HashMap;

/// Unique identifier for a Shape.
pub type ShapeId = u32;

/// Property descriptor attributes (ECMAScript standard).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyAttributes {
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}

impl Default for PropertyAttributes {
    fn default() -> Self {
        Self { writable: true, enumerable: true, configurable: true }
    }
}

/// Metadata about a property within a shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyInfo {
    pub offset: u32,
    pub attributes: PropertyAttributes,
}

/// A hidden class representing the property layout of a JavaScript object.
#[derive(Debug, Clone)]
pub struct Shape {
    pub id: ShapeId,
    pub parent: Option<ShapeId>,
    pub property_offsets: HashMap<String, PropertyInfo>,
    pub transitions: HashMap<String, ShapeId>,
    pub next_offset: u32,
}

impl Shape {
    pub fn new_root(id: ShapeId) -> Self {
        Self {
            id,
            parent: None,
            property_offsets: HashMap::new(),
            transitions: HashMap::new(),
            next_offset: 0,
        }
    }

    #[inline]
    pub fn lookup(&self, key: &str) -> Option<u32> {
        self.property_offsets.get(key).map(|info| info.offset)
    }
}

/// Central registry managing all shapes and transitions within an isolate or VM.
#[derive(Debug, Clone)]
pub struct ShapeRegistry {
    shapes: Vec<Shape>,
}

impl ShapeRegistry {
    pub const ROOT_SHAPE_ID: ShapeId = 0;

    pub fn new() -> Self {
        let mut registry = Self { shapes: Vec::with_capacity(64) };
        // Shape 0 is the empty object shape
        registry.shapes.push(Shape::new_root(Self::ROOT_SHAPE_ID));
        registry
    }

    #[inline]
    pub fn get(&self, id: ShapeId) -> Option<&Shape> {
        self.shapes.get(id as usize)
    }

    #[inline]
    pub fn lookup_offset(&self, shape_id: ShapeId, key: &str) -> Option<u32> {
        self.get(shape_id)?.lookup(key)
    }

    /// Transition from `current_shape_id` by adding property `key`.
    /// Returns `(new_shape_id, property_offset)`.
    pub fn transition(&mut self, current_shape_id: ShapeId, key: &str) -> (ShapeId, u32) {
        if let Some(&existing_child_id) =
            self.shapes[current_shape_id as usize].transitions.get(key)
        {
            let offset = self.shapes[existing_child_id as usize]
                .lookup(key)
                .expect("Child shape must contain transitioned property");
            return (existing_child_id, offset);
        }

        let parent = &self.shapes[current_shape_id as usize];
        let offset = parent.next_offset;
        let mut property_offsets = parent.property_offsets.clone();
        property_offsets
            .insert(key.to_string(), PropertyInfo { offset, attributes: PropertyAttributes::default() });

        let new_id = self.shapes.len() as ShapeId;
        let new_shape = Shape {
            id: new_id,
            parent: Some(current_shape_id),
            property_offsets,
            transitions: HashMap::new(),
            next_offset: offset + 1,
        };

        self.shapes.push(new_shape);
        self.shapes[current_shape_id as usize]
            .transitions
            .insert(key.to_string(), new_id);

        (new_id, offset)
    }
    /// Check if shape has a property.
    #[inline]
    pub fn has_property(&self, shape_id: ShapeId, key: &str) -> bool {
        self.lookup_offset(shape_id, key).is_some()
    }

    /// Return the count of properties in a shape.
    #[inline]
    pub fn property_count(&self, shape_id: ShapeId) -> usize {
        self.get(shape_id).map(|s| s.property_offsets.len()).unwrap_or(0)
    }

    /// Check if `shape_id` is a descendant of `ancestor_id` in the transition tree.
    pub fn is_descendant_of(&self, mut shape_id: ShapeId, ancestor_id: ShapeId) -> bool {
        while let Some(shape) = self.get(shape_id) {
            if shape.id == ancestor_id {
                return true;
            }
            if let Some(parent) = shape.parent {
                shape_id = parent;
            } else {
                break;
            }
        }
        false
    }

    /// Return all property names defined in this shape in insertion order.
    pub fn all_properties(&self, shape_id: ShapeId) -> Vec<String> {
        if let Some(s) = self.get(shape_id) {
            let mut props: Vec<(&String, u32)> = s.property_offsets.iter().map(|(k, info)| (k, info.offset)).collect();
            props.sort_by_key(|(_, off)| *off);
            props.into_iter().map(|(k, _)| k.clone()).collect()
        } else {
            Vec::new()
        }
    }

    /// Remove a property from a shape, returning a new shape ID without the property.
    pub fn remove_property(&mut self, current_shape_id: ShapeId, key: &str) -> ShapeId {
        let parent = match self.get(current_shape_id) {
            Some(p) => p,
            None => return current_shape_id,
        };
        if !parent.property_offsets.contains_key(key) {
            return current_shape_id;
        }
        let mut property_offsets = parent.property_offsets.clone();
        property_offsets.remove(key);
        let next_offset = parent.next_offset;
        let new_id = self.shapes.len() as ShapeId;
        let new_shape = Shape {
            id: new_id,
            parent: Some(current_shape_id),
            property_offsets,
            transitions: HashMap::new(),
            next_offset,
        };
        self.shapes.push(new_shape);
        new_id
    }
}

impl Default for ShapeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_transitions() {
        let mut registry = ShapeRegistry::new();
        let (s1, off_x) = registry.transition(ShapeRegistry::ROOT_SHAPE_ID, "x");
        assert_eq!(off_x, 0);

        let (s2, off_y) = registry.transition(s1, "y");
        assert_eq!(off_y, 1);

        // Transition with same property on same shape reuses child shape
        let (s1_again, off_x2) = registry.transition(ShapeRegistry::ROOT_SHAPE_ID, "x");
        assert_eq!(s1, s1_again);
        assert_eq!(off_x, off_x2);

        assert_eq!(registry.lookup_offset(s2, "x"), Some(0));
        assert_eq!(registry.lookup_offset(s2, "y"), Some(1));
        assert_eq!(registry.lookup_offset(s2, "z"), None);

        assert!(registry.has_property(s2, "x"));
        assert!(registry.has_property(s2, "y"));
        assert!(!registry.has_property(s2, "z"));
        assert_eq!(registry.property_count(s2), 2);
        assert!(registry.is_descendant_of(s2, s1));
        assert!(registry.is_descendant_of(s2, ShapeRegistry::ROOT_SHAPE_ID));
    }
}
