//! Fast dispatch for `Map`/`Set` prototype methods.

use super::*;

mod map;
mod set_basic;
mod set_ops;

impl Vm {
    pub(crate) fn get_map_method(&mut self, target: JsValue, name: &str) -> Option<JsValue> {
        match name {
            "get" => map::get(self, target),
            "set" => map::set(self, target),
            "has" => map::has(self, target),
            "delete" => map::delete(self, target),
            "clear" => map::clear(self, target),
            "forEach" => map::for_each(self, target),
            "entries" => map::entries(self, target),
            "keys" => map::keys(self, target),
            "values" => map::values(self, target),
            _ => None,
        }
    }

    pub(crate) fn get_set_method(&mut self, target: JsValue, name: &str) -> Option<JsValue> {
        match name {
            "values" | "keys" => set_basic::values_or_keys(self, target),
            "entries" => set_basic::entries(self, target),
            "add" => set_basic::add(self, target),
            "has" => set_basic::has(self, target),
            "delete" => set_basic::delete(self, target),
            "clear" => set_basic::clear(self, target),
            "forEach" => set_basic::for_each(self, target),
            "union" => set_ops::union(self, target),
            "intersection" => set_ops::intersection(self, target),
            "difference" => set_ops::difference(self, target),
            "symmetricDifference" => set_ops::symmetric_difference(self, target),
            "isSubsetOf" => set_ops::is_subset_of(self, target),
            "isSupersetOf" => set_ops::is_superset_of(self, target),
            "isDisjointFrom" => set_ops::is_disjoint_from(self, target),
            _ => None,
        }
    }
}
