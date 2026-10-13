//! Variable scopes and symbol tables for the bytecode compiler.

#[derive(Debug, Clone)]
pub struct Local {
    pub name: String,
    pub depth: usize,
    pub slot: u16,
    pub is_lexical: bool,
}

/// Information about a captured upvalue in lexical scoping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpvalueInfo {
    pub index: u16,
    pub is_local: bool,
    pub name: String,
}

#[derive(Debug, Default, Clone)]
pub struct SymbolTable {
    pub locals: Vec<Local>,
    pub all_local_names: Vec<String>,
    pub scope_depth: usize,
    pub upvalues: Vec<UpvalueInfo>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enter_scope(&mut self) {
        self.scope_depth += 1;
    }

    pub fn exit_scope(&mut self) -> usize {
        let mut popped = 0;
        while let Some(last) = self.locals.last() {
            if last.depth == self.scope_depth {
                self.locals.pop();
                popped += 1;
            } else {
                break;
            }
        }
        self.scope_depth -= 1;
        popped
    }

    pub fn define_local(&mut self, name: String) -> u16 {
        let is_lexical = self.scope_depth > 1;
        let slot = self.all_local_names.len() as u16;
        self.all_local_names.push(name.clone());
        self.locals.push(Local { name, depth: self.scope_depth, slot, is_lexical });
        slot
    }

    pub fn define_local_lexical(&mut self, name: String) -> u16 {
        let slot = self.all_local_names.len() as u16;
        self.all_local_names.push(name.clone());
        self.locals.push(Local { name, depth: self.scope_depth, slot, is_lexical: true });
        slot
    }

    pub fn define_local_at_depth(&mut self, name: String, depth: usize) -> u16 {
        let slot = self.all_local_names.len() as u16;
        self.all_local_names.push(name.clone());
        self.locals.push(Local { name, depth, slot, is_lexical: false });
        slot
    }

    pub fn resolve_local(&self, name: &str) -> Option<u16> {
        for local in self.locals.iter().rev() {
            if local.name == name {
                return Some(local.slot);
            }
        }
        None
    }

    pub fn is_local_in_current_scope(&self, name: &str) -> bool {
        for local in self.locals.iter().rev() {
            if local.depth < self.scope_depth {
                break;
            }
            if local.name == name {
                return true;
            }
        }
        false
    }

    pub fn add_upvalue(&mut self, index: u16, is_local: bool, name: &str) -> u16 {
        for (i, uv) in self.upvalues.iter().enumerate() {
            if uv.index == index && uv.is_local == is_local && uv.name == name {
                return i as u16;
            }
        }
        let slot = self.upvalues.len() as u16;
        self.upvalues.push(UpvalueInfo {
            index,
            is_local,
            name: name.to_string(),
        });
        slot
    }

    pub fn resolve_upvalue(&mut self, name: &str, parent: &mut SymbolTable) -> Option<u16> {
        if let Some(local_idx) = parent.resolve_local(name) {
            return Some(self.add_upvalue(local_idx, true, name));
        }
        if let Some(parent_uv_idx) = parent.find_upvalue(name) {
            return Some(self.add_upvalue(parent_uv_idx, false, name));
        }
        None
    }

    pub fn find_upvalue(&self, name: &str) -> Option<u16> {
        for (i, uv) in self.upvalues.iter().enumerate() {
            if uv.name == name {
                return Some(i as u16);
            }
        }
        None
    }
}

