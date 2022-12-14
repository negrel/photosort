use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::FromUtf8Error;
use std::{error, fmt};

use serde::de::Visitor;
use serde::Deserialize;
use thiserror::Error;

pub mod context;
pub mod variables;

use context::Context;

/// Template define a simple PathBuf template engine.
///
/// Template is a template engine that only supports variable substitution (no branching, loop,
/// etc). It makes uses of Context to get and render variables (implementing []).
#[derive(Debug, Clone)]
pub struct Template {
    tokens: Vec<Token>,
}

#[derive(Debug, Clone)]
enum Token {
    String(String),
    Variable(String),
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ParseError {
    #[error("unamed variable (at index {0})")]
    UnamedVariable(usize),
    #[error("unclosed variable (at index {0})")]
    UnclosedVariable(usize),
}

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("undefined variable {0:?}")]
    UndefinedVariable(String),

    #[error("failed to build string")]
    BuildString(#[from] FromUtf8Error),

    #[error("failed to render \"{0}\" variable: {1}")]
    VariableRender(String, #[source] Box<dyn error::Error>),
}

impl Template {
    pub fn render(&self, ctx: &Context) -> Result<PathBuf, RenderError> {
        let mut result = OsString::default();

        for i in 0..self.tokens.len() {
            let tk = &self.tokens[i];

            match tk {
                Token::String(str) => result.push(&str[..]),
                Token::Variable(name) => {
                    if let Some(value) = ctx.get(name) {
                        let rendered_value = match value.render(name, ctx) {
                            Ok(v) => v,
                            Err(err) => {
                                return Err(RenderError::VariableRender(name.to_owned(), err))
                            }
                        };
                        result.push(rendered_value);
                    } else {
                        return Err(RenderError::UndefinedVariable(name.to_string()));
                    }
                }
            }
        }

        Ok(PathBuf::from(result))
    }
}

impl FromStr for Template {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = Vec::new();
        let mut char_count = 1;

        let mut variable_start_index: Option<usize> = None;
        let mut string_start_index: Option<usize> = Some(0);
        for (i, c) in s.chars().peekable().enumerate() {
            char_count += 1;
            let is_variable_delimiter = c == ':';

            if is_variable_delimiter {
                if let Some(start_str) = string_start_index {
                    if start_str != i {
                        tokens.push(Token::String(String::from(&s[start_str..i])));
                    }

                    variable_start_index = Some(i + 1);
                    string_start_index = None;
                } else if let Some(start_var) = variable_start_index {
                    if start_var == i {
                        return Err(ParseError::UnamedVariable(i));
                    }

                    tokens.push(Token::Variable(s[start_var..i].to_string()));
                    string_start_index = Some(i + 1);
                    variable_start_index = None;
                }
            }
        }

        if let Some(start_str) = string_start_index {
            // Last string value
            if start_str < char_count - 1 {
                tokens.push(Token::String(String::from(&s[start_str..])));
            }
        } else if variable_start_index.is_some() {
            // Last value is a variable
            return Err(ParseError::UnclosedVariable(s.len() - 1));
        } else if tokens.is_empty() && !s.is_empty() {
            tokens.push(Token::String(String::from(s)))
        }

        Ok(Template { tokens })
    }
}

impl<'de> Deserialize<'de> for Template {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TemplateVisitor;
        impl<'de> Visitor<'de> for TemplateVisitor {
            type Value = Template;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a template literal string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match Template::from_str(v) {
                    Ok(template) => Ok(template),
                    Err(err) => Err(E::custom(format!(
                        "failed to deserialize template: {}",
                        err
                    ))),
                }
            }
        }

        deserializer.deserialize_str(TemplateVisitor {})
    }
}

#[cfg(test)]
mod tests {
    use thiserror::Error;

    use crate::template::context::TemplateValue;

    use super::context::Context;
    use super::{ParseError, RenderError, Template};
    use std::{path::PathBuf, str::FromStr};

    #[test]
    fn string_without_variable() {
        let tpl = Template::from_str("abcdef").unwrap();
        assert_eq!(tpl.tokens.len(), 1);

        let str = tpl.render(&Context::default()).unwrap();
        assert_eq!(str, PathBuf::from("abcdef"));
        let str = tpl.render(&Context::default()).unwrap();
        assert_eq!(str, PathBuf::from("abcdef"));

        let mut ctx = Context::default();
        let unused_var = "Hello world".to_owned();
        ctx.insert(&["k"], Box::new(unused_var));
        let str = tpl.render(&ctx).unwrap();
        assert_eq!(str, PathBuf::from("abcdef"));
    }

    #[test]
    fn empty_string() {
        let tpl = Template::from_str("").unwrap();
        assert_eq!(tpl.tokens.len(), 0);

        let str = tpl.render(&Context::default()).unwrap();
        assert_eq!(str, PathBuf::from(""));
        let str = tpl.render(&Context::default()).unwrap();
        assert_eq!(str, PathBuf::from(""));
    }

    #[test]
    fn string() {
        let tpl = Template::from_str(":date.day:/constant_prefix:date.month:/:date.year:").unwrap();
        assert_eq!(tpl.tokens.len(), 5);

        let mut ctx = Context::default();
        let year = "2022";
        ctx.insert(&["date.year"], Box::new(year));
        let month = "08";
        ctx.insert(&["date.month"], Box::new(month));
        let day = "19";
        ctx.insert(&["date.day"], Box::new(day));

        let str = tpl.render(&ctx).unwrap();
        assert_eq!(str, PathBuf::from("19/constant_prefix08/2022"));

        let str = tpl.render(&ctx).unwrap();
        assert_eq!(str, PathBuf::from("19/constant_prefix08/2022"));
    }

    #[test]
    fn string_with_unclosed_variable_error() {
        let tpl = Template::from_str(":date.day");
        assert_eq!(tpl.unwrap_err(), ParseError::UnclosedVariable(8));
    }

    #[test]
    fn string_with_unnamed_variable_error() {
        let tpl = Template::from_str("i'm going to :: next year");
        assert_eq!(tpl.unwrap_err(), ParseError::UnamedVariable(14));
    }

    #[test]
    fn undefined_variable_error() {
        let tpl = Template::from_str("i'm going to :destination: next year").unwrap();
        let result = tpl.render(&Context::default());
        let render_err = result.unwrap_err();

        match render_err {
            RenderError::UndefinedVariable(variable) => {
                assert_eq!(variable, "destination".to_string())
            }
            _ => panic!(
                "expected error of type UndefinedVariable, got {}",
                render_err
            ),
        }
    }

    #[test]
    fn variable_render_error() {
        #[derive(Error, Debug)]
        enum SimpleError {
            #[error("an error occurred")]
            A(),
        }
        struct AlwaysFailTemplateValue {}
        impl TemplateValue for AlwaysFailTemplateValue {
            fn render(&self, _name: &str, _ctx: &Context) -> crate::template::context::Result {
                Err(Box::new(SimpleError::A()))
            }
        }

        let tpl = Template::from_str("a :simple.variable: !").unwrap();
        let mut ctx = Context::default();
        ctx.insert(&["simple.variable"], Box::new(AlwaysFailTemplateValue {}));

        let result = tpl.render(&ctx);
        let render_err = result.unwrap_err();

        match render_err {
            RenderError::VariableRender(variable, error) => {
                assert_eq!("simple.variable", variable);
                assert_eq!(error.to_string(), "an error occurred");
            }
            _ => panic!("expected error of type VariableRender, got {}", render_err),
        }
    }
}
