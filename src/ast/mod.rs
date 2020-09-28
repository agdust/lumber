//! Internal representations of all components of a Lumber source file/program.
#[macro_use]
mod macros;

mod alias;
mod arity;
mod atom;
mod body;
mod conjunction;
mod definition;
mod disjunction;
mod expression;
mod handle;
mod identifier;
mod implication;
mod literal;
mod module;
mod pattern;
mod query;
mod scope;
mod r#struct;
mod unification;

pub(crate) use alias::Alias;
pub(crate) use arity::Arity;
pub(crate) use atom::Atom;
pub(crate) use body::Body;
pub(crate) use conjunction::Conjunction;
pub(crate) use definition::Definition;
pub(crate) use disjunction::Disjunction;
pub(crate) use expression::Expression;
pub(crate) use handle::{AsHandle, Handle};
pub(crate) use identifier::Identifier;
pub(crate) use implication::Implication;
pub(crate) use literal::Literal;
pub(crate) use module::Module;
pub(crate) use pattern::Pattern;
pub(crate) use query::Query;
pub(crate) use r#struct::Struct;
pub(crate) use scope::Scope;
pub(crate) use unification::Unification;

mod builtin;
mod context;
mod fields;
mod module_header;
mod prec_climber;

pub(crate) use context::Context;
pub(crate) use fields::fields;
pub(crate) use module_header::ModuleHeader;
pub(crate) use prec_climber::{Operator, PrecClimber};

#[cfg(test)]
mod test;
