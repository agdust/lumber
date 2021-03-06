use crate::Binding;

mod database;
mod patterns;

type Bindings<'a> = Box<dyn Iterator<Item = Binding> + 'a>;

pub(crate) use patterns::unify_patterns;
