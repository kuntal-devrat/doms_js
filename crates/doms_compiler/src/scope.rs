//! Variable scopes and symbol tables for the bytecode compiler.

#[derive(Debug, Clone)]
pub struct Local {
    pub name: String,
    pub depth: usize,
}

#[derive(Debug, Default)]
pub struct SymbolTable {
    pub locals: Vec<Local>,
    pub scope_depth: usize,
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
        let slot = self.locals.len() as u16;
        self.locals.push(Local { name, depth: self.scope_depth });
        slot
    }

    pub fn resolve_local(&self, name: &str) -> Option<u16> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i as u16);
            }
        }
        None
    }
}
