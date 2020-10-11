use super::super::{Database, DatabaseDefinition};
use super::{unify_patterns, Bindings};
use crate::ast::*;
use crate::{Binding, Question};

impl Database<'_> {
    pub(crate) fn unify_question<'a>(
        &'a self,
        question: &'a Question,
    ) -> impl Iterator<Item = Binding> + 'a {
        let body = question.as_ref();
        let binding = body.identifiers().collect();
        self.unify_body(body, binding, true)
    }

    fn unify_body<'a>(&'a self, body: &'a Body, binding: Binding, public: bool) -> Bindings<'a> {
        match &body.0 {
            Some(disjunction) => self.unify_disjunction(disjunction, binding, public),
            None => Box::new(std::iter::once(binding)),
        }
    }

    fn unify_disjunction<'a>(
        &'a self,
        disjunction: &'a Disjunction,
        binding: Binding,
        public: bool,
    ) -> Bindings<'a> {
        disjunction
            .cases
            .iter()
            .find_map(move |case| -> Option<Bindings> {
                let mut bindings = self
                    .unify_conjunction(case, binding.clone(), public)
                    .peekable();
                bindings.peek()?;
                Some(Box::new(bindings))
            })
            .unwrap_or(Box::new(std::iter::empty()))
    }

    fn unify_conjunction<'a>(
        &'a self,
        conjunction: &'a Conjunction,
        binding: Binding,
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
        binding: Binding,
        public: bool,
    ) -> Bindings<'a> {
        let bindings = Box::new(std::iter::once(binding.clone()));
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
        binding: Binding,
        public: bool,
    ) -> Bindings<'a> {
        match unification {
            Unification::Never => return Box::new(std::iter::empty()),
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
                                    Some(unify_patterns(&lhs, rhs, binding, &[])?.1)
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
                        Some(unify_patterns(&output, &pattern, binding, &[])?.1)
                    }),
            ),
        }
    }

    fn unify_definition<'a>(
        &'a self,
        query: &'a Query,
        definition: &'a Definition,
        input_binding: Binding,
    ) -> Bindings<'a> {
        Box::new(definition.iter().flat_map(move |(head, body)| {
            let input_binding = input_binding.clone();
            body.identifiers()
                .collect::<Binding>()
                .transfer_from(&input_binding, &query, &head)
                .map(move |binding| self.unify_body(body, binding, false))
                .into_iter()
                .flatten()
                .filter_map(move |output_binding| {
                    input_binding
                        .clone()
                        .transfer_from(&output_binding, &head, &query)
                })
        }))
    }

    fn unify_expression<'a>(
        &'a self,
        expression: &'a Expression,
        binding: Binding,
        public: bool,
    ) -> Box<dyn Iterator<Item = (Binding, Pattern)> + 'a> {
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
