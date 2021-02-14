use super::*;
use crate::ast;
use std::fmt::{self, Display, Formatter};

/// A sequence of narrowing steps.
#[derive(Default, Clone, Debug)]
pub(crate) struct Procession {
    /// Steps after which backtracking is skipped.
    pub(crate) steps: Vec<Step>,
}

impl Procession {
    pub fn handles_mut(&mut self) -> impl Iterator<Item = &mut Handle> {
        self.steps.iter_mut().flat_map(|step| step.handles_mut())
    }
}

impl Display for Procession {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for (i, step) in self.steps.iter().enumerate() {
            if i != 0 {
                write!(f, " -> ")?;
            }
            step.fmt(f)?;
        }
        Ok(())
    }
}

impl From<ast::Procession> for Procession {
    fn from(ast: ast::Procession) -> Procession {
        Procession {
            steps: ast.steps.into_iter().map(Step::from).collect(),
        }
    }
}

impl Variables for Procession {
    fn variables(&self, vars: &mut Vec<Variable>) {
        for step in &self.steps {
            step.variables(vars)
        }
    }
}
