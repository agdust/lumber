use super::super::{Database, DatabaseDefinition};
use super::{unify_patterns, Bindings};
use crate::ast::*;
use crate::{Binding, Question};
use std::borrow::Cow;

impl Database<'_> {
    pub(crate) fn unify_question<'a>(
        &'a self,
        question: &'a Question,
    ) -> impl Iterator<Item = Binding> + 'a {
        let body = question.as_ref();
        self.unify_body(body, Cow::Borrowed(&question.initial_binding), true)
            .map(|cow| cow.into_owned()) // TODO: do we even need to owned it here?
    }

    fn unify_body<'a>(
        &'a self,
        body: &'a Body,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Bindings<'a> {
        match &body.0 {
            Some(disjunction) => self.unify_disjunction(disjunction, binding, public),
            None => Box::new(std::iter::once(binding)),
        }
    }

    fn unify_disjunction<'a>(
        &'a self,
        disjunction: &'a Disjunction,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Bindings<'a> {
        Box::new(
            disjunction
                .cases
                .iter()
                .flat_map(move |(head, tail)| {
                    let head_bindings = self.unify_conjunction(head, binding.clone(), public);
                    match tail {
                        None => Box::new(head_bindings.map(|binding| (binding, false)))
                            as Box<dyn Iterator<Item = (Cow<'a, Binding>, bool)>>,
                        Some(tail) => Box::new(
                            head_bindings
                                .flat_map(move |binding| {
                                    self.unify_conjunction(tail, binding, public)
                                })
                                .map(|binding| (binding, true)),
                        ),
                    }
                })
                .scan(false, |stop, (binding, done)| {
                    if *stop {
                        None
                    } else {
                        *stop = done;
                        Some(binding)
                    }
                })
                .fuse(),
        )
    }

    fn unify_conjunction<'a>(
        &'a self,
        conjunction: &'a Conjunction,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Bindings<'a> {
        let bindings = Box::new(std::iter::once(binding));
        conjunction.terms.iter().fold(bindings, |bindings, term| {
            Box::new(bindings.flat_map(move |binding| self.unify_procession(term, binding, public)))
        })
    }

    fn unify_procession<'a>(
        &'a self,
        procession: &'a Procession,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Bindings<'a> {
        let bindings = Box::new(std::iter::once(binding));
        procession
            .steps
            .iter()
            .fold(bindings, |mut bindings, step| match bindings.next() {
                Some(binding) => self.perform_unification(step, binding, public),
                None => Box::new(std::iter::empty()),
            })
    }

    fn perform_unification<'a>(
        &'a self,
        unification: &'a Unification,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Bindings<'a> {
        match unification {
            Unification::Query(query) => {
                let definition = match self.lookup(query.as_ref(), public) {
                    Some(definition) => definition,
                    None => return Box::new(std::iter::empty()),
                };
                match definition {
                    DatabaseDefinition::Static(definition) => {
                        self.unify_definition(&query, definition, binding)
                    }
                    DatabaseDefinition::Mutable(_definition) => {
                        todo!("Not sure yet how mutable definitions can be handled soundly")
                        // self.unify_definition(&query, &*definition.borrow(), binding)
                    }
                    DatabaseDefinition::Native(native_function) => {
                        let values = query
                            .patterns
                            .iter()
                            .map(|pattern| binding.extract(pattern).unwrap())
                            .collect::<Vec<_>>();
                        Box::new(native_function.call(values).filter_map(move |values| {
                            let values = values
                                .into_iter()
                                .map(Into::into)
                                .zip(query.patterns.iter())
                                .try_fold(binding.clone(), |binding, (lhs, rhs)| {
                                    Some(unify_patterns(
                                        Cow::Borrowed(&lhs),
                                        Cow::Borrowed(rhs),
                                        binding,
                                        &[],
                                    )?)
                                });
                            values
                        }))
                    }
                    _ => unreachable!(),
                }
            }
            Unification::Body(body) => self.unify_body(body, binding, public),
            Unification::Assumption(output, expression) => Box::new(
                self.unify_expression(expression, binding, public)
                    .filter_map(move |(binding, pattern)| {
                        Some(unify_patterns(
                            Cow::Borrowed(&output),
                            Cow::Owned(pattern),
                            binding,
                            &[],
                        )?)
                    }),
            ),
        }
    }

    fn unify_definition<'a>(
        &'a self,
        query: &'a Query,
        definition: &'a Definition,
        input_binding: Cow<'a, Binding>,
    ) -> Bindings<'a> {
        Box::new(
            definition
                .iter()
                .flat_map(move |(head, kind, body)| {
                    let input_binding = input_binding.clone();
                    let output_binding = head
                        .identifiers()
                        .chain(body.identifiers())
                        .collect::<Binding>();
                    Binding::transfer_from(
                        Cow::Owned(output_binding),
                        &input_binding,
                        &query,
                        &head,
                    )
                    .map(move |binding| self.unify_body(body, binding, false))
                    .into_iter()
                    .flatten()
                    .filter_map(move |output_binding| {
                        Binding::transfer_from(
                            input_binding.clone(),
                            &output_binding,
                            &head,
                            &query,
                        )
                    })
                    .map(move |output| (output, *kind))
                })
                .scan(RuleKind::Multi, |kind, (value, newkind)| {
                    if let RuleKind::Once = kind {
                        return None;
                    }
                    *kind = newkind;
                    Some(value)
                })
                .fuse(),
        )
    }

    fn unify_expression<'a>(
        &'a self,
        expression: &'a Expression,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Box<dyn Iterator<Item = (Cow<'a, Binding>, Pattern)> + 'a> {
        match expression {
            Expression::Operation(pattern, unifications) => Box::new(
                unifications
                    .iter()
                    .fold(
                        Box::new(std::iter::once(binding)) as Bindings,
                        |bindings: Bindings, term: &Unification| -> Bindings {
                            Box::new(bindings.flat_map(move |binding| {
                                self.perform_unification(term, binding, public)
                            }))
                        },
                    )
                    .map(move |binding| (binding, pattern.clone())),
            ),
            Expression::Value(pattern) => Box::new(std::iter::once((binding, pattern.clone()))),
            #[cfg(feature = "builtin-sets")]
            Expression::SetAggregation(pattern, body) => {
                let solutions = self
                    .unify_disjunction(&body.0, binding.clone(), public)
                    .map(|binding| binding.apply(&pattern).unwrap())
                    .collect();
                Box::new(std::iter::once((binding, Pattern::Set(solutions, None))))
            }
            Expression::ListAggregation(pattern, body) => {
                let solutions = self
                    .unify_body(body, binding.clone(), public)
                    .map(|binding| binding.apply(&pattern).unwrap())
                    .collect();
                Box::new(std::iter::once((binding, Pattern::List(solutions, None))))
            }
        }
    }
}
