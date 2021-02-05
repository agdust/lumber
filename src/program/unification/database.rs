use super::super::{Database, DatabaseDefinition};
use super::evaltree::*;
use super::{unify_patterns, Binding, Bindings};
use crate::Question;
use std::borrow::Cow;

type Evaluation<'a> = (Pattern, Bindings<'a>);
type MultipleEvaluations<'a> = (Vec<Pattern>, Cow<'a, Binding>);

#[cfg(feature = "test-perf")]
struct FlameIterator<I>(I, usize);

#[cfg(feature = "test-perf")]
impl<I> Iterator for FlameIterator<I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.1 += 1;
        flame::start("FlameIterator::next");
        let output = self.0.next();
        flame::end("FlameIterator::next");
        flame::dump_html(std::fs::File::create(format!("Flame-{}.html", self.1)).unwrap()).unwrap();
        flame::clear();
        output
    }
}

impl Database<'_> {
    #[cfg_attr(feature = "test-perf", flamer::flame)]
    pub(crate) fn unify_question<'a>(
        &'a self,
        question: &'a Question,
    ) -> impl Iterator<Item = Binding> + 'a {
        let body = question.as_ref();
        let answers = self
            .unify_body(body, Cow::Borrowed(&question.initial_binding), true)
            .map(|cow| cow.into_owned()); // TODO: do we even need to owned it here?
        #[cfg(feature = "test-perf")]
        {
            FlameIterator(answers, 0)
        }
        #[cfg(not(feature = "test-perf"))]
        {
            answers
        }
    }

    /// Runs a test. A test does not need to reference public predicates only.
    #[cfg_attr(feature = "test-perf", flamer::flame)]
    pub(crate) fn unify_test<'a>(
        &'a self,
        question: &'a Question,
    ) -> impl Iterator<Item = Binding> + 'a {
        let body = question.as_ref();
        let answers = self
            .unify_body(body, Cow::Borrowed(&question.initial_binding), false)
            .map(|cow| cow.into_owned());
        answers
    }

    #[cfg_attr(feature = "test-perf", flamer::flame)]
    fn unify_body<'a>(
        &'a self,
        body: &'a Body,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Bindings<'a> {
        self.unify_disjunction(&body.0, binding, public)
    }

    #[cfg_attr(feature = "test-perf", flamer::flame)]
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
                .map(move |(head, tail)| {
                    let head_bindings = self.unify_conjunction(head, binding.clone(), public);
                    match tail {
                        None => (Box::new(head_bindings), None),
                        Some(tail) => (Box::new(head_bindings), Some(tail)),
                    }
                })
                .scan(false, |skip_rest, (mut head, tail)| {
                    if *skip_rest {
                        return None;
                    }
                    if let Some(binding) = head.next() {
                        if tail.is_some() {
                            *skip_rest = true;
                            Some((
                                Box::new(std::iter::once(binding))
                                    as Box<dyn Iterator<Item = Cow<'a, Binding>>>,
                                tail,
                            ))
                        } else {
                            Some((Box::new(std::iter::once(binding).chain(head)), tail))
                        }
                    } else {
                        Some((Box::new(std::iter::empty()), tail))
                    }
                })
                .fuse()
                .flat_map(move |(head_bindings, tail)| -> Bindings<'a> {
                    match tail {
                        None => Box::new(head_bindings),
                        Some(tail) => Box::new(head_bindings.flat_map(move |binding| {
                            self.unify_conjunction(tail, binding, public)
                        })),
                    }
                }),
        )
    }

    #[cfg_attr(feature = "test-perf", flamer::flame)]
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

    #[cfg_attr(feature = "test-perf", flamer::flame)]
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
                Some(binding) => self.perform_step(step, binding, public),
                None => Box::new(std::iter::empty()),
            })
    }

    #[cfg_attr(feature = "test-perf", flamer::flame)]
    fn perform_step<'a>(
        &'a self,
        unification: &'a Step,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Bindings<'a> {
        match unification {
            Step::Query(query) => Box::new(
                self.evaluate_expressions(query.args(), binding, public)
                    .flat_map(move |(arguments, binding)| {
                        self.unify_query(query.handle(), arguments, binding, public)
                    }),
            ),
            Step::Relation(None, op, rhs) => {
                match self.resolve_operator(&OpKey::Relation(op.clone(), OpArity::Unary)) {
                    Some(operator) => {
                        let handle = operator.handle();
                        Box::new(
                            self.evaluate_term(rhs, binding, public)
                                .into_iter()
                                .flat_map(move |(pattern, bindings)| {
                                    bindings.flat_map(move |binding| {
                                        self.unify_query(
                                            handle,
                                            vec![pattern.clone()],
                                            binding,
                                            false,
                                        )
                                    })
                                }),
                        )
                    }
                    None => Box::new(std::iter::empty()),
                }
            }
            Step::Relation(Some(lhs), op, rhs) => {
                match self.resolve_operator(&OpKey::Relation(op.clone(), OpArity::Binary)) {
                    Some(operator) => {
                        let handle = operator.handle();
                        Box::new(
                            self.evaluate_term(lhs, binding, public)
                                .into_iter()
                                .flat_map(move |(lvar, bindings)| {
                                    bindings.into_iter().flat_map(move |binding| {
                                        let lvar = lvar.clone();
                                        self.evaluate_term(rhs, binding, public)
                                            .into_iter()
                                            .flat_map(move |(rvar, bindings)| {
                                                bindings.flat_map({
                                                    let lvar = lvar.clone();
                                                    move |binding| {
                                                        self.unify_query(
                                                            handle,
                                                            vec![lvar.clone(), rvar.clone()],
                                                            binding,
                                                            false,
                                                        )
                                                    }
                                                })
                                            })
                                    })
                                }),
                        )
                    }
                    None => Box::new(std::iter::empty()),
                }
            }
            Step::Body(body) => self.unify_body(body, binding, public),
            Step::Unification(lhs, rhs) => Box::new(
                self.evaluate_expression(lhs, binding, public)
                    .into_iter()
                    .flat_map(move |(lvar, bindings)| {
                        bindings.flat_map(move |binding| {
                            self.evaluate_expression(rhs, binding, public)
                                .into_iter()
                                .flat_map({
                                    let lvar = lvar.clone();
                                    move |(rvar, bindings)| {
                                        bindings.flat_map({
                                            let lvar = lvar.clone();
                                            move |binding| {
                                                unify_patterns(lvar.clone(), rvar.clone(), binding)
                                            }
                                        })
                                    }
                                })
                        })
                    }),
            ),
        }
    }

    fn evaluate_expressions<'a>(
        &'a self,
        expressions: &'a [Expression],
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> impl Iterator<Item = MultipleEvaluations<'a>> {
        expressions.iter().fold(
            Box::new(std::iter::once((vec![], binding))) as Box<dyn Iterator<Item = _>>,
            move |bindings: Box<dyn Iterator<Item = MultipleEvaluations<'a>>>, expression| {
                Box::new(bindings.flat_map(move |(outputs, binding)| {
                    self.evaluate_expression(expression, binding, public)
                        .into_iter()
                        .flat_map(move |(var, bindings)| {
                            let mut outputs = outputs.clone();
                            outputs.push(var);
                            bindings.map(move |binding| (outputs.clone(), binding))
                        })
                }))
            },
        )
    }

    fn unify_query<'a>(
        &'a self,
        handle: &Handle,
        args: Vec<Pattern>,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Bindings<'a> {
        assert_eq!(handle.arity.len() as usize, args.len());
        let definition = match self.lookup(handle, public) {
            Some(definition) => definition,
            None => return Box::new(std::iter::empty()),
        };
        match definition {
            DatabaseDefinition::Static(definition) => {
                self.unify_definition(definition, args, binding)
            }
            DatabaseDefinition::Mutable(_definition) => {
                todo!("Not sure yet how mutable definitions can be handled soundly")
            }
            DatabaseDefinition::Native(native_function) => {
                let values = args.iter().map(|p| binding.extract(p).unwrap()).collect();
                Box::new(native_function.call(values).filter_map(move |values| {
                    args.iter().cloned().zip(values.into_iter()).try_fold(
                        binding.clone(),
                        |mut binding, (lhs, rhs)| {
                            let rhs = binding.to_mut().associate_value(rhs);
                            unify_patterns(lhs, rhs, binding)
                        },
                    )
                }))
            }
            _ => unreachable!(),
        }
    }

    #[cfg_attr(feature = "test-perf", flamer::flame)]
    fn unify_definition<'a>(
        &'a self,
        definition: &'a Definition,
        expressions: Vec<Pattern>,
        input_binding: Cow<'a, Binding>,
    ) -> Bindings<'a> {
        Box::new(
            definition
                .iter()
                .map({
                    let input_binding = input_binding.clone();
                    move |(head, kind, body)| {
                        let output_binding = input_binding.start_generation(
                            body.as_ref(),
                            &expressions,
                            &head.patterns.to_vec(),
                        );
                        (output_binding, *kind, body)
                    }
                })
                .scan(false, |skip_rest, (binding, kind, body)| {
                    if *skip_rest {
                        return None;
                    }
                    if binding.is_some() && kind == RuleKind::Once {
                        *skip_rest = true;
                    }
                    Some((binding, body))
                })
                .fuse()
                .flat_map(move |(binding, body)| {
                    Box::new(
                        binding
                            .map(move |binding| match body {
                                Some(body) => self.unify_body(body, binding, false),
                                None => Box::new(std::iter::once(binding)),
                            })
                            .into_iter()
                            .flatten()
                            .map(|binding| Cow::Owned(binding.into_owned().end_generation())),
                    )
                }),
        )
    }

    #[cfg_attr(feature = "test-perf", flamer::flame)]
    fn evaluate_expression<'a>(
        &'a self,
        expression: &'a Expression,
        binding: Cow<'a, Binding>,
        public: bool,
    ) -> Option<Evaluation<'a>> {
        let eval = expression
            .climb_operators::<Box<dyn Fn(Cow<'a, Binding>) -> Option<Evaluation<'a>>>, _, _, _, _, _>(
                |operator| self.resolve_operator(operator),
                move |term| Box::new(move |binding| self.evaluate_term(term, binding, public)),
                move |term, operator| {
                    Box::new(move |mut binding| {
                        let dest = Pattern::from(PatternKind::Variable(binding.to_mut().fresh_variable()));
                        let (out, bindings) = term(binding)?;
                        let bindings =
                            Box::new(bindings.flat_map({ let dest = dest.clone(); move |binding| {
                                self.unify_query(
                                    operator.handle(),
                                    vec![out.clone(), dest.clone()],
                                    binding,
                                    public,
                                )
                            }}));
                        Some((
                            dest,
                            bindings,
                        ))
                    })
                },
                move |lhs, operator, rhs| {
                    let rhs = std::rc::Rc::new(rhs);
                    Box::new(move |mut binding| {
                        let dest = Pattern::from(PatternKind::Variable(binding.to_mut().fresh_variable()));
                        let (lvar, bindings) = lhs(binding)?;
                        let bindings = Box::new(bindings.flat_map({
                            let rhs = rhs.clone();
                            let dest = dest.clone();
                            move |binding| {
                                rhs(binding)
                                    .into_iter()
                                    .flat_map({
                                        let lvar = lvar.clone();
                                        let dest = dest.clone();
                                        move |(rvar, bindings)| {
                                            bindings.flat_map({
                                                let lvar = lvar.clone();
                                                let dest = dest.clone();
                                                move |binding| {
                                                    self.unify_query(
                                                        operator.handle(),
                                                        vec![
                                                            lvar.clone(),
                                                            rvar.clone(),
                                                            dest.clone(),
                                                        ],
                                                        binding,
                                                        public,
                                                    )
                                                }
                                            })
                                        }
                                    })
                            }
                        }));
                        Some((dest, bindings))
                    })
                },
            );
        (eval?)(binding)
    }

    #[cfg_attr(feature = "test-perf", flamer::flame)]
    fn evaluate_term<'a>(
        &'a self,
        term: &'a Term,
        mut binding: Cow<'a, Binding>,
        public: bool,
    ) -> Option<Evaluation<'a>> {
        match term {
            Term::Expression(expression) => self.evaluate_expression(expression, binding, public),
            Term::PrefixOp(op, rhs) => {
                let dest = Pattern::from(PatternKind::Variable(binding.to_mut().fresh_variable()));
                let (rvar, bindings) = self.evaluate_term(rhs, binding, public)?;
                let bindings = Box::new(bindings.flat_map({
                    let dest = dest.clone();
                    move |binding| {
                        self.unify_query(
                            op.handle(),
                            vec![rvar.clone(), dest.clone()],
                            binding,
                            public,
                        )
                    }
                }));
                Some((dest, bindings))
            }
            Term::InfixOp(lhs, op, rhs) => {
                let dest = Pattern::from(PatternKind::Variable(binding.to_mut().fresh_variable()));
                let (lvar, bindings) = self.evaluate_term(lhs, binding, public)?;
                let bindings = Box::new(bindings.flat_map({
                    let dest = dest.clone();
                    move |binding| {
                        self.evaluate_term(rhs, binding, public)
                            .into_iter()
                            .flat_map({
                                let dest = dest.clone();
                                let lvar = lvar.clone();
                                move |(rvar, bindings)| {
                                    bindings.flat_map({
                                        let lvar = lvar.clone();
                                        let dest = dest.clone();
                                        move |binding| {
                                            self.unify_query(
                                                op.handle(),
                                                vec![lvar.clone(), rvar.clone(), dest.clone()],
                                                binding,
                                                public,
                                            )
                                        }
                                    })
                                }
                            })
                    }
                }));
                Some((dest, bindings))
            }
            Term::Value(pattern) => Some((pattern.clone(), Box::new(std::iter::once(binding)))),
            Term::ListAggregation(pattern, body) => {
                let solutions = self
                    .unify_body(body, binding.clone(), public)
                    .map(move |binding| binding.extract(&pattern).unwrap())
                    .map(|value| binding.to_mut().associate_value(value))
                    .collect();
                Some((
                    Pattern::list(solutions, None),
                    Box::new(std::iter::once(binding)),
                ))
            }
        }
    }
}
