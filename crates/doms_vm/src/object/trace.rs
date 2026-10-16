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
            }
            ObjectKind::Map(entries) => {
                for (k, v) in entries {
                    if let Some(p) = k.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                    if let Some(p) = v.as_heap_ptr() {
                        tracer.trace_ref(p);
                    }
                }
            }
            ObjectKind::Set(elements) => {
                for el in elements {
                    if let Some(p) = el.as_heap_ptr() {
                        tracer.trace_ref(p);
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
            _ => {}
        }
    }
}
