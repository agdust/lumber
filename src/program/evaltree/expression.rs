use super::*;
use crate::ast;
use crate::climb::*;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Debug)]
pub(crate) struct Expression(Vec<Op<Atom, Term>>);

impl Expression {
    pub fn handles_mut<'a>(&'a mut self) -> Box<dyn Iterator<Item = &mut Handle> + 'a> {
        Box::new(
            self.0
                .iter_mut()
                .flat_map(|op| -> Box<dyn Iterator<Item = &mut Handle>> {
                    match op {
                        Op::Rator(..) => Box::new(std::iter::empty()),
                        Op::Rand(term) => term.handles_mut(),
                    }
                }),
        )
    }

    pub fn identifiers<'a>(&'a self) -> Box<dyn Iterator<Item = Identifier> + 'a> {
        Box::new(
            self.0
                .iter()
                .filter_map(|op| match op {
                    Op::Rand(term) => Some(term),
                    _ => None,
                })
                .flat_map(|term| term.identifiers()),
        )
    }

    pub fn resolve_operators<F: FnMut(&OpKey) -> Option<Operator>>(&mut self, resolve: F) {
        if let Some(term) = self.climb_operators(
            resolve,
            Clone::clone,
            Term::prefix_operator,
            Term::infix_operator,
        ) {
            self.0 = vec![Op::Rand(term)];
        }
    }

    pub fn climb_operators<
        'a,
        Out,
        Resolved: Climbable,
        Res: FnMut(&OpKey) -> Option<Resolved>,
        Init: Fn(&'a Term) -> Out,
        Prefix: Fn(Out, Resolved) -> Out,
        Infix: Copy + Fn(Out, Resolved, Out) -> Out,
    >(
        &'a self,
        mut resolve: Res,
        init: Init,
        prefix: Prefix,
        infix: Infix,
    ) -> Option<Out> {
        let mut collapsed = vec![];
        // Resolve unary operators
        let mut arity = OpArity::Unary;
        let mut prefixes = vec![];
        for op in &self.0 {
            match op {
                Op::Rator(name) if arity == OpArity::Unary => {
                    let operator = resolve(&OpKey::Expression(name.clone(), arity))?;
                    prefixes.push(operator);
                }
                Op::Rator(name) => {
                    let operator = resolve(&OpKey::Expression(name.clone(), arity))?;
                    arity = OpArity::Unary;
                    collapsed.push(Op::Rator(operator));
                }
                Op::Rand(term) => {
                    arity = OpArity::Binary;
                    let reduced = prefixes.drain(..).rev().fold(init(term), &prefix);
                    collapsed.push(Op::Rand(reduced));
                }
            }
        }

        Some(climb(collapsed.into_iter(), infix))
    }
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join(" ")
            .fmt(f)
    }
}

impl<T> From<T> for Expression
where
    Term: From<T>,
{
    fn from(value: T) -> Self {
        Self(vec![Op::Rand(Term::from(value))])
    }
}

impl From<ast::Expression> for Expression {
    fn from(ast: ast::Expression) -> Self {
        Self(
            ast.0
                .into_iter()
                .map(|op| match op {
                    Op::Rator(o) => Op::Rator(o),
                    Op::Rand(t) => Op::Rand(Term::from(t)),
                })
                .collect(),
        )
    }
}
