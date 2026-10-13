//! Hidden classes (Shapes) for fast property access.

use std::collections::{HashMap, HashSet};

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

/// Transition that created this shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeTransition {
    Root,
    Add {
        key: String,
        offset: u32,
        attributes: PropertyAttributes,
    },
    Remove {
        key: String,
    },
}

/// A hidden class representing the property layout of a JavaScript object.
/// Shapes form a tree rooted at Shape 0 (empty object).
/// Instead of duplicating all property mappings across every shape node,
/// each shape node only stores its own single transition. This avoids O(N^2)
/// memory explosion when objects define thousands of properties.
#[derive(Debug, Clone)]
pub struct Shape {
    pub id: ShapeId,
    pub parent: Option<ShapeId>,
    pub transition: ShapeTransition,
    pub next_offset: u32,
    pub num_properties: usize,
    pub transitions: HashMap<String, ShapeId>,
    pub delete_transitions: HashMap<String, ShapeId>,
}

impl Shape {
    pub fn new_root(id: ShapeId) -> Self {
        Self {
            id,
            parent: None,
            transition: ShapeTransition::Root,
            next_offset: 0,
            num_properties: 0,
            transitions: HashMap::new(),
            delete_transitions: HashMap::new(),
        }
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

    /// Retrieve the property information (offset and attributes) for `key` on `shape_id`.
    pub fn get_property_info(&self, mut shape_id: ShapeId, key: &str) -> Option<PropertyInfo> {
        let mut deleted: Option<Vec<&str>> = None;
        while let Some(shape) = self.shapes.get(shape_id as usize) {
            match &shape.transition {
                ShapeTransition::Add { key: prop_key, offset, attributes } => {
                    if prop_key == key {
                        if let Some(del) = &deleted {
                            if del.iter().any(|d| *d == key) {
                                return None;
                            }
                        }
                        return Some(PropertyInfo {
                            offset: *offset,
                            attributes: *attributes,
                        });
                    }
                }
                ShapeTransition::Remove { key: del_key } => {
                    if del_key == key {
                        return None;
                    }
                    deleted.get_or_insert_with(Vec::new).push(del_key.as_str());
                }
                ShapeTransition::Root => break,
            }
            match shape.parent {
                Some(p) => shape_id = p,
                None => break,
            }
        }
        None
    }

    #[inline]
    pub fn lookup_offset(&self, shape_id: ShapeId, key: &str) -> Option<u32> {
        self.get_property_info(shape_id, key).map(|info| info.offset)
    }

    /// Transition from `current_shape_id` by adding property `key`.
    /// Returns `(new_shape_id, property_offset)`.
    pub fn transition(&mut self, current_shape_id: ShapeId, key: &str) -> (ShapeId, u32) {
        if let Some(&existing_child_id) =
            self.shapes[current_shape_id as usize].transitions.get(key)
        {
            if let ShapeTransition::Add { offset, .. } =
                self.shapes[existing_child_id as usize].transition
            {
                return (existing_child_id, offset);
            }
        }

        let parent = &self.shapes[current_shape_id as usize];
        let offset = parent.next_offset;
        let new_id = self.shapes.len() as ShapeId;
        let new_shape = Shape {
            id: new_id,
            parent: Some(current_shape_id),
            transition: ShapeTransition::Add {
                key: key.to_string(),
                offset,
                attributes: PropertyAttributes::default(),
            },
            next_offset: offset + 1,
            num_properties: parent.num_properties + 1,
            transitions: HashMap::new(),
            delete_transitions: HashMap::new(),
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
        self.get(shape_id).map(|s| s.num_properties).unwrap_or(0)
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

    /// Return all property (key, offset) pairs defined in this shape in insertion order.
    pub fn all_property_entries(&self, mut shape_id: ShapeId) -> Vec<(String, u32)> {
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        let mut deleted: Option<HashSet<String>> = None;

        while let Some(shape) = self.shapes.get(shape_id as usize) {
            match &shape.transition {
                ShapeTransition::Add { key, offset, .. } => {
                    let is_deleted = deleted.as_ref().map(|d| d.contains(key)).unwrap_or(false);
                    if !is_deleted && seen.insert(key.clone()) {
                        entries.push((key.clone(), *offset));
                    }
                }
                ShapeTransition::Remove { key } => {
                    deleted.get_or_insert_with(HashSet::new).insert(key.clone());
                }
                ShapeTransition::Root => break,
            }
            match shape.parent {
                Some(p) => shape_id = p,
                None => break,
            }
        }

        entries.reverse();
        entries
    }

    /// Return all property names defined in this shape in insertion order.
    pub fn all_properties(&self, shape_id: ShapeId) -> Vec<String> {
        self.all_property_entries(shape_id).into_iter().map(|(k, _)| k).collect()
    }

    /// Remove a property from a shape, returning a new shape ID without the property.
    pub fn remove_property(&mut self, current_shape_id: ShapeId, key: &str) -> ShapeId {
        if !self.has_property(current_shape_id, key) {
            return current_shape_id;
        }

        if let Some(&existing_child_id) =
            self.shapes[current_shape_id as usize].delete_transitions.get(key)
        {
            return existing_child_id;
        }

        let parent = &self.shapes[current_shape_id as usize];
        let new_id = self.shapes.len() as ShapeId;
        let new_shape = Shape {
            id: new_id,
            parent: Some(current_shape_id),
            transition: ShapeTransition::Remove { key: key.to_string() },
            next_offset: parent.next_offset,
            num_properties: parent.num_properties.saturating_sub(1),
            transitions: HashMap::new(),
            delete_transitions: HashMap::new(),
        };

        self.shapes.push(new_shape);
        self.shapes[current_shape_id as usize]
            .delete_transitions
            .insert(key.to_string(), new_id);

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

        // Test property removal and ordering
        let s3 = registry.remove_property(s2, "x");
        assert_eq!(registry.property_count(s3), 1);
        assert!(!registry.has_property(s3, "x"));
        assert!(registry.has_property(s3, "y"));
        assert_eq!(registry.all_properties(s3), vec!["y".to_string()]);

        // Re-adding property
        let (s4, off_x3) = registry.transition(s3, "x");
        assert_eq!(off_x3, 2);
        assert_eq!(registry.property_count(s4), 2);
        assert_eq!(registry.all_properties(s4), vec!["y".to_string(), "x".to_string()]);
    }
}
