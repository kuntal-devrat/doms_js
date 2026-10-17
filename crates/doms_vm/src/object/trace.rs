//! GC tracing for `JsObject`.

use super::*;

impl Trace for JsObject {
    fn trace(&self, tracer: &mut Tracer) {
        if let Some(proto) = self.prototype {
            if let Some(p) = proto.as_heap_ptr() {
                tracer.trace_ref(p);
            }
        }
        for slot in &self.slots {
            if let Some(p) = slot.as_heap_ptr() {
                tracer.trace_ref(p);
            }
        }
        if let Some(accs) = &self.accessors {
            for acc in accs.values() {
                if let Some(g) = acc.getter {
                    if let Some(p) = g.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
                if let Some(s) = acc.setter {
                    if let Some(p) = s.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
        }
        if let Some(syms) = &self.symbol_properties {
            for (k, v) in syms.iter() {
                if let Some(p) = k.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
                if let Some(p) = v.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
            }
        }
        if let Some(sym_accs) = &self.symbol_accessors {
            for (k, acc) in sym_accs.iter() {
                if let Some(p) = k.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
                if let Some(g) = acc.getter {
                    if let Some(p) = g.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
                if let Some(s) = acc.setter {
                    if let Some(p) = s.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
        }
        if let Some(order) = &self.symbol_order {
            for k in order {
                if let Some(p) = k.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
            }
        }
        if let Some(privs) = &self.private_elements {
            for v in privs.values() {
                match v {
                    PrivateElement::Value(val) => {
                        if let Some(p) = val.as_heap_ptr() {
                            tracer.trace_ref(p);
                        }
                    }
                    PrivateElement::Accessor(acc) => {
                        if let Some(g) = acc.getter {
                            if let Some(p) = g.as_heap_ptr() {
                                tracer.trace_ref(p);
                            }
                        }
                        if let Some(s) = acc.setter {
                            if let Some(p) = s.as_heap_ptr() {
                                tracer.trace_ref(p);
                            }
                        }
                    }
                }
            }
        }
        match &self.kind {
            ObjectKind::Array(elements) => {
                for el in elements {
                    if let Some(p) = el.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            ObjectKind::Function(f) => {
                for upval in &f.upvalues {
                    if let Some(p) = upval.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
                if let Some(ho) = f.home_object {
                    if let Some(p) = ho.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            ObjectKind::Promise(p) => {
                if let Some(r) = p.result.as_heap_ptr() {
                    tracer.trace_ref(r);
                }
                for cb in &p.fulfill_reactions {
                    if let Some(r) = cb.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
                for cb in &p.reject_reactions {
                    if let Some(r) = cb.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
            }
            ObjectKind::Generator(g) => {
                for v in &g.stack {
                    if let Some(r) = v.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
                for v in &g.locals {
                    if let Some(r) = v.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
                for sc in &g.scopes {
                    if let Some(r) = sc.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
                if let Some(env) = g.env_obj {
                    if let Some(r) = env.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
                for uv in &g.function.upvalues {
                    if let Some(r) = uv.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
                if let Some(ho) = g.function.home_object {
                    if let Some(r) = ho.as_heap_ptr() {
                        tracer.trace_ref(r);
                    }
                }
            }
            ObjectKind::Map(entries) => {
                for entry in entries {
                    if let Some((k, v)) = entry {
                        if let Some(p) = k.as_heap_ptr() {
                            tracer.trace_ref(p);
                        }
                        if let Some(p) = v.as_heap_ptr() {
                            tracer.trace_ref(p);
                        }
                    }
                }
            }
            ObjectKind::Set(elements) => {
                for el in elements {
                    if let Some(el) = el {
                        if let Some(p) = el.as_heap_ptr() {
                            tracer.trace_ref(p);
                        }
                    }
                }
            }
            ObjectKind::WeakMap(entries) => {
                for (k, v) in entries {
                    if let Some(p) = k.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                    if let Some(p) = v.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            ObjectKind::WeakSet(elements) => {
                for el in elements {
                    if let Some(p) = el.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            ObjectKind::FinalizationRegistry { callback, entries } => {
                if let Some(p) = callback.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
                for (_, held, tok) in entries {
                    if let Some(p) = held.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                    if let Some(t) = tok {
                        if let Some(p) = t.as_heap_ptr() {
                            tracer.trace_ref(p);
                        }
                    }
                }
            }
            ObjectKind::Proxy { target, handler } => {
                if let Some(p) = target.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
                if let Some(p) = handler.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
            }
            ObjectKind::IteratorRecord { iterator, next_method, .. }
            | ObjectKind::AsyncIteratorRecord { iterator, next_method, .. } => {
                if let Some(p) = iterator.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
                if let Some(p) = next_method.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
            }
            ObjectKind::MapIterator { map, .. } => {
                if let Some(p) = map.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
            }
            ObjectKind::SetIterator { set, .. } => {
                if let Some(p) = set.as_heap_ptr() {
                    tracer.trace_ref(p);
                }
            }
            _ => {}
        }
    }
}

impl JsObject {
    /// Mutably visit all internal `JsValue` references in this object.
    ///
    /// Used during GC scavenging and root preservation.
    pub fn visit_values_mut<F>(&mut self, mut visitor: F)
    where
        F: FnMut(&mut JsValue),
    {
        if let Some(proto) = &mut self.prototype {
            visitor(proto);
        }
        for slot in &mut self.slots {
            visitor(slot);
        }
        if let Some(accs) = &mut self.accessors {
            for acc in accs.values_mut() {
                if let Some(g) = &mut acc.getter {
                    visitor(g);
                }
                if let Some(s) = &mut acc.setter {
                    visitor(s);
                }
            }
        }
        if let Some(syms) = &mut self.symbol_properties {
            let mut updated = std::collections::HashMap::with_capacity(syms.len());
            for (mut k, mut v) in syms.drain() {
                visitor(&mut k);
                visitor(&mut v);
                updated.insert(k, v);
            }
            **syms = updated;
        }
        if let Some(sym_accs) = &mut self.symbol_accessors {
            let mut updated = std::collections::HashMap::with_capacity(sym_accs.len());
            for (mut k, mut acc) in sym_accs.drain() {
                visitor(&mut k);
                if let Some(g) = &mut acc.getter {
                    visitor(g);
                }
                if let Some(s) = &mut acc.setter {
                    visitor(s);
                }
                updated.insert(k, acc);
            }
            **sym_accs = updated;
        }
        if let Some(order) = &mut self.symbol_order {
            for k in order.iter_mut() {
                visitor(k);
            }
        }
        match &mut self.kind {
            ObjectKind::Array(elements) => {
                for el in elements {
                    visitor(el);
                }
            }
            ObjectKind::Proxy { target, handler } => {
                visitor(target);
                visitor(handler);
            }
            ObjectKind::WeakRef(target) => {
                visitor(target);
            }
            ObjectKind::Function(f) => {
                for uv in &mut f.upvalues {
                    visitor(uv);
                }
                if let Some(ref mut ho) = f.home_object {
                    visitor(ho);
                }
            }
            ObjectKind::Promise(p) => {
                visitor(&mut p.result);
                for r in &mut p.fulfill_reactions {
                    visitor(r);
                }
                for r in &mut p.reject_reactions {
                    visitor(r);
                }
            }
            ObjectKind::Generator(g) => {
                for v in &mut g.stack {
                    visitor(v);
                }
                for v in &mut g.locals {
                    visitor(v);
                }
                for sc in &mut g.scopes {
                    visitor(sc);
                }
                if let Some(env) = &mut g.env_obj {
                    visitor(env);
                }
                for uv in &mut g.function.upvalues {
                    visitor(uv);
                }
                if let Some(ref mut ho) = g.function.home_object {
                    visitor(ho);
                }
                visitor(&mut g.this_val);
                if let Some(args) = &mut g.arguments {
                    visitor(args);
                }
            }
            ObjectKind::Map(entries) => {
                for entry in entries {
                    if let Some((k, v)) = entry {
                        visitor(k);
                        visitor(v);
                    }
                }
            }
            ObjectKind::Set(elements) => {
                for el in elements {
                    if let Some(el) = el {
                        visitor(el);
                    }
                }
            }
            ObjectKind::WeakMap(entries) => {
                for (k, v) in entries {
                    visitor(k);
                    visitor(v);
                }
            }
            ObjectKind::WeakSet(elements) => {
                for el in elements {
                    visitor(el);
                }
            }
            ObjectKind::FinalizationRegistry { callback, entries } => {
                visitor(callback);
                for (t, h, unreg) in entries {
                    visitor(t);
                    visitor(h);
                    if let Some(u) = unreg {
                        visitor(u);
                    }
                }
            }
            ObjectKind::IteratorRecord { iterator, next_method, .. }
            | ObjectKind::AsyncIteratorRecord { iterator, next_method, .. } => {
                visitor(iterator);
                visitor(next_method);
            }
            ObjectKind::MapIterator { map, .. } => {
                visitor(map);
            }
            ObjectKind::SetIterator { set, .. } => {
                visitor(set);
            }
            _ => {}
        }
    }
}
