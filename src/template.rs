use std::collections::HashMap;
use std::hash::BuildHasher;
use std::string::FromUtf8Error;

use string_builder::Builder;
use thiserror::Error;

pub trait TemplateValue {
    fn render(&self) -> String;
}

impl TemplateValue for dyn ToString {
    fn render(&self) -> String {
        self.to_string()
    }
}

impl TemplateValue for String {
    fn render(&self) -> String {
        self.to_owned()
    }
}

#[derive(Debug)]
pub struct Template<'a> {
    tokens: Vec<Token<'a>>,
}

#[derive(Debug)]
enum Token<'a> {
    String(String),
    Variable(&'a str),
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnamedVariable,
    UnclosedVariable,
}

#[derive(Error, Debug, PartialEq)]
pub enum RenderError {
    #[error("undefined variable {0:?}")]
    UndefinedVariable(String),

    #[error("failed to build string")]
    BuildString(#[from] FromUtf8Error),
}

impl<'a> Template<'a> {
    pub fn parse_str(s: &'a str) -> Result<Self, ParseError> {
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
                        return Err(ParseError::UnamedVariable);
                    }

                    tokens.push(Token::Variable(&s[start_var..i]));
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
        } else if let Some(_) = variable_start_index {
            // Last value is a variable
            return Err(ParseError::UnclosedVariable);
        } else if tokens.len() == 0 && s.len() > 0 {
            tokens.push(Token::String(String::from(s)))
        }

        Ok(Template::<'a> { tokens })
    }

    pub fn render<'v, T: BuildHasher>(
        &self,
        variables: &HashMap<&str, &'v dyn TemplateValue, T>,
    ) -> Result<String, RenderError> {
        let mut builder = Builder::default();

        for i in 0..self.tokens.len() {
            let tk = &self.tokens[i];

            match tk {
                Token::String(str) => builder.append(&str[..]),
                Token::Variable(name) => {
                    if let Some(value) = variables.get(name) {
                        builder.append(value.render());
                    } else {
                        return Err(RenderError::UndefinedVariable(name.to_string()));
                    }
                }
            }
        }

        builder.string().map_err(|err| err.into())
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseError, RenderError, Template, TemplateValue};
    use std::collections::HashMap;

    #[test]
    fn string_without_variable() {
        let tpl = Template::parse_str("abcdef").unwrap();
        assert_eq!(tpl.tokens.len(), 1);

        let str = tpl.render(&HashMap::new()).unwrap();
        assert_eq!(str, "abcdef");
        let str = tpl.render(&HashMap::new()).unwrap();
        assert_eq!(str, "abcdef");

        let mut hmap: HashMap<&str, &dyn TemplateValue> = HashMap::new();
        let unused_var = "Hello world".to_owned();
        hmap.insert("k", &unused_var);
        let str = tpl.render(&hmap).unwrap();
        assert_eq!(str, "abcdef");
    }

    #[test]
    fn empty_string() {
        let tpl = Template::parse_str("").unwrap();
        assert_eq!(tpl.tokens.len(), 0);

        let str = tpl.render(&HashMap::new()).unwrap();
        assert_eq!(str, "");
        let str = tpl.render(&HashMap::new()).unwrap();
        assert_eq!(str, "");
    }

    #[test]
    fn string() {
        let tpl = Template::parse_str(":date.day:/0:date.month:/:date.year:").unwrap();
        assert_eq!(tpl.tokens.len(), 5);

        let mut hmap: HashMap<&str, &dyn TemplateValue> = HashMap::new();
        let year = 2022.to_string();
        hmap.insert("date.year", &year);
        let month = 08.to_string();
        hmap.insert("date.month", &month);
        let day = 19.to_string();
        hmap.insert("date.day", &day);

        let str = tpl.render(&hmap).unwrap();
        assert_eq!(str, "19/08/2022");

        let str = tpl.render(&hmap).unwrap();
        assert_eq!(str, "19/08/2022");
    }

    #[test]
    fn string_with_unclosed_variable_error() {
        let tpl = Template::parse_str(":date.day");
        assert_eq!(tpl.unwrap_err(), ParseError::UnclosedVariable);
    }

    #[test]
    fn string_with_unnamed_variable_error() {
        let tpl = Template::parse_str("i'm going to :: next year");
        assert_eq!(tpl.unwrap_err(), ParseError::UnamedVariable);
    }

    #[test]
    fn undefined_variable_error() {
        let tpl = Template::parse_str("i'm going to :destination: next year").unwrap();
        let result = tpl.render(&HashMap::new());

        assert_eq!(
            result.unwrap_err(),
            RenderError::UndefinedVariable("destination".to_string())
        );
    }
}
