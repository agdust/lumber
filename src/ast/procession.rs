use super::*;
use crate::parser::Rule;

/// A sequence of steps.
#[derive(Default, Clone, Debug)]
pub(crate) struct Procession {
    /// Steps between which backtracking is skipped..
    pub(crate) steps: Vec<Unification>,
}

impl Procession {
    pub fn new(pair: crate::Pair, context: &mut Context) -> Option<Self> {
        assert_eq!(pair.as_rule(), Rule::procession);
        let steps = pair
            .into_inner()
            .map(|pair| Unification::new(pair, context))
            .collect::<Option<_>>()?;
        Some(Self { steps })
    }

    pub fn handles_mut(&mut self) -> impl Iterator<Item = &mut Handle> {
        self.steps.iter_mut().flat_map(|step| step.handles_mut())
    }

    pub fn identifiers<'a>(&'a self) -> impl Iterator<Item = Identifier> + 'a {
        self.steps.iter().flat_map(|step| step.identifiers())
    }
}
