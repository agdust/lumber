use crate::ast::*;
use crate::parser::*;
use crate::{Binding, Value};
use std::collections::BTreeMap;
use std::convert::TryFrom;

/// A question ready to be asked to the Lumber program.
///
/// These can be constructed from strings using [`Question::try_from`][].
pub struct Question {
    body: Body,
}

impl AsRef<Body> for Question {
    fn as_ref(&self) -> &Body {
        &self.body
    }
}

impl Question {
    /// Uses a binding to extract the answer to this question.
    pub fn answer(&self, binding: &Binding) -> Option<BTreeMap<String, Option<Value>>> {
        self.body
            .identifiers()
            .map(|identifier| {
                Some((
                    identifier.name().to_owned(),
                    binding.extract(binding.get(&identifier)?).ok()?,
                ))
            })
            .collect()
    }
}

impl TryFrom<&str> for Question {
    type Error = crate::Error;

    /// A string using Lumber syntax can be converted directly into a question. It is not recommended
    /// to construct questions dynamically in this way, as the error will not be recoverable. There is
    /// not currently another method of constructing questions, but it is a planned feature to have
    /// some sort of question builder, DSL, or derive-based solution for this problem.
    ///
    /// For one-off statically written questions, string conversions should be fine and unwrapped.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use lumber::Question;
    /// use std::convert::TryInto;
    ///
    /// let question: Question = "test(A)".try_into().unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Will return an error if the syntax is invalid.
    fn try_from(src: &str) -> crate::Result<Question> {
        let mut pairs = Parser::parse_question(src)?;
        let pair = pairs.next().unwrap();
        assert_eq!(Rule::question, pair.as_rule());
        let mut pairs = pair.into_inner();
        let pair = pairs.next().unwrap();
        let mut context = Context::default();
        let body = Body::new(pair, &mut context).unwrap();
        Ok(Question { body })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn question_from_str_single() {
        Question::try_from("hello(A)").unwrap();
    }

    #[test]
    fn question_from_str_scoped() {
        Question::try_from("hello::world(A)").unwrap();
    }

    #[test]
    #[should_panic]
    fn question_from_str_parent() {
        Question::try_from("^::hello(A)").unwrap();
    }

    #[test]
    #[should_panic]
    fn question_from_str_punctuated() {
        Question::try_from("hello(A).").unwrap();
    }

    #[test]
    fn question_from_str_multi() {
        Question::try_from(
            "hello(A) -> hello(B), hello(C); hello(C), hello(D) -> hello(E), F <- 3",
        )
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn question_empty() {
        Question::try_from("").unwrap();
    }
}
