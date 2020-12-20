use super::*;
use crate::parser::Rule;
use std::fmt::{self, Display, Formatter};

/// A unification against the database, used to build up a rule.
#[derive(Clone, Debug)]
pub(crate) enum Step {
    /// A single query to be unified with the database.
    Query(Query),
    /// A query represented by an operator.
    Relation(Option<Term>, Atom, Term),
    /// An entire sub-rule of unifications to be made.
    Body(Body),
    /// A direcct unification.
    Unification(Expression, Expression),
}

impl Step {
    pub fn new(pair: crate::Pair, context: &mut Context) -> Option<Self> {
        assert_eq!(pair.as_rule(), Rule::step);
        let pair = just!(pair.into_inner());
        let step = match pair.as_rule() {
            Rule::unification => Self::from_unification(pair, context)?,
            Rule::predicate => Self::Query(Query::new(pair, context)?),
            Rule::disjunction => Self::Body(Body::new_inner(pair, context)?),
            Rule::relation => Self::from_relation(pair, context)?,
            _ => unreachable!(),
        };
        Some(step)
    }

    fn from_unification(pair: crate::Pair, context: &mut Context) -> Option<Self> {
        assert_eq!(pair.as_rule(), Rule::unification);
        let mut pairs = pair.into_inner();
        let lhs = Expression::new(pairs.next().unwrap(), context)?;
        let rhs = Expression::new(pairs.next().unwrap(), context)?;
        Some(Self::Unification(lhs, rhs))
    }

    fn from_relation(pair: crate::Pair, context: &mut Context) -> Option<Self> {
        let mut pairs = pair.into_inner();
        let pair = pairs.next().unwrap();
        let (lhs, operator) = match pair.as_rule() {
            Rule::term => {
                let lhs = Term::new(pair, context)?;
                let operator = Atom::from(pairs.next().unwrap().as_str());
                (Some(lhs), operator)
            }
            Rule::operator => {
                let operator = Atom::from(pair.as_str());
                (None, operator)
            }
            _ => unreachable!(),
        };
        let rhs = Term::new(pairs.next().unwrap(), context)?;
        Some(Self::Relation(lhs, operator, rhs))
    }

    pub fn handles_mut<'a>(&'a mut self) -> Box<dyn Iterator<Item = &mut Handle> + 'a> {
        match self {
            Self::Query(query) => Box::new(std::iter::once(query.as_mut())),
            Self::Body(body) => Box::new(body.handles_mut()),
            Self::Relation(lhs, _, rhs) => Box::new(
                lhs.iter_mut()
                    .flat_map(Term::handles_mut)
                    .chain(rhs.handles_mut()),
            ),
            Self::Unification(lhs, rhs) => Box::new(lhs.handles_mut().chain(rhs.handles_mut())),
        }
    }

    pub fn resolve_operators<F: FnMut(&OpKey) -> Option<Operator>>(&mut self, mut resolve: F) {
        match self {
            Self::Relation(Some(lhs), operator, rhs) => {
                match resolve(&OpKey::Relation(operator.clone(), OpArity::Binary)) {
                    Some(operator) => {
                        *self = Self::Query(Query {
                            handle: operator.handle().clone(),
                            args: vec![lhs.clone().into(), rhs.clone().into()],
                        });
                    }
                    None => {} // an error should be recorded in the context
                }
            }
            Self::Relation(None, operator, rhs) => {
                match resolve(&OpKey::Relation(operator.clone(), OpArity::Unary)) {
                    Some(operator) => {
                        *self = Self::Query(Query {
                            handle: operator.handle().clone(),
                            args: vec![rhs.clone().into()],
                        });
                    }
                    None => {} // an error should be recorded in the context
                }
            }
            Self::Query(query) => query
                .args_mut()
                .for_each(|expr| expr.resolve_operators(&mut resolve)),
            Self::Unification(lhs, rhs) => {
                lhs.resolve_operators(&mut resolve);
                rhs.resolve_operators(&mut resolve);
            }
            _ => {}
        }
    }

    pub fn identifiers<'a>(&'a self) -> Box<dyn Iterator<Item = Identifier> + 'a> {
        match self {
            Self::Query(query) => Box::new(query.identifiers()),
            Self::Body(body) => Box::new(body.identifiers()),
            Self::Relation(lhs, _, rhs) => Box::new(
                lhs.iter()
                    .flat_map(Term::identifiers)
                    .chain(rhs.identifiers()),
            ),
            Self::Unification(pattern, expression) => {
                Box::new(pattern.identifiers().chain(expression.identifiers()))
            }
        }
    }
}

impl Display for Step {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Query(query) => query.fmt(f),
            Self::Body(body) => write!(f, "({})", body),
            Self::Relation(Some(lhs), operator, rhs) => write!(f, "{} {} {}", lhs, operator, rhs),
            Self::Relation(None, operator, rhs) => write!(f, "{}{}", operator, rhs),
            Self::Unification(lhs, rhs) => write!(f, "{} =:= {}", lhs, rhs),
        }
    }
}
