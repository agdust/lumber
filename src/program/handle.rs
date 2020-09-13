use super::*;
use crate::parser::Rule;
use std::fmt::{self, Display, Formatter};

/// A handle to a predicate.
#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Handle {
    /// The path and name of the predicate or function being described
    scope: Scope,
    /// The arity of the predicate or function being described
    arity: Vec<Arity>,
}

impl Handle {
    pub(crate) fn new<'i>(pair: crate::Pair<'i>, context: &mut Context<'i>) -> Self {
        Self::new_in_scope(context.current_scope.clone(), pair, context)
    }

    pub(crate) fn new_in_scope<'i>(
        mut scope: Scope,
        pair: crate::Pair<'i>,
        context: &mut Context<'i>,
    ) -> Self {
        assert_eq!(pair.as_rule(), Rule::handle);
        let mut pairs = pair.into_inner();
        let atom = context.atomizer.atomize(pairs.next().unwrap());
        scope.push(atom);
        let arity = pairs.map(|pair| Arity::new(pair, context)).collect();
        Self { scope, arity }
    }
}

impl Display for Handle {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.scope.fmt(f)?;
        for arity in &self.arity {
            arity.fmt(f)?;
        }
        Ok(())
    }
}
