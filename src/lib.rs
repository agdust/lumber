//! Lumber is a logic programming language, mainly intended to be embedded in Rust programs.
//!
//! More info will be available soon.

mod ast;
mod core;
mod lumber;
mod parser;
mod program;

mod error;

pub use error::{Error, ErrorKind};
pub type Result<T> = std::result::Result<T, Error>;

type Pairs<'i> = pest::iterators::Pairs<'i, parser::Rule>;
type Pair<'i> = pest::iterators::Pair<'i, parser::Rule>;

pub use crate::lumber::{Lumber, LumberBuilder};
