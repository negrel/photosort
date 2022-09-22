use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::FromUtf8Error;

use thiserror::Error;

pub trait TemplateValue {
    fn render(&self, name: &str, ctx: &dyn Context) -> OsString;
}

impl TemplateValue for dyn ToString {
    fn render(&self, name: &str, ctx: &dyn Context) -> OsString {
        self.to_string().render(name, ctx)
    }
}

impl TemplateValue for &str {
    fn render(&self, name: &str, ctx: &dyn Context) -> OsString {
        self.to_owned().to_owned().render(name, ctx)
    }
}

impl TemplateValue for String {
    fn render(&self, _name: &str, _ctx: &dyn Context) -> OsString {
        OsString::from_str(self).unwrap()
    }
}

impl TemplateValue for PathBuf {
    fn render(&self, _name: &str, _ctx: &dyn Context) -> OsString {
        self.clone().into_os_string()
    }
}

impl TemplateValue for OsString {
    fn render(&self, _name: &str, _ctx: &dyn Context) -> OsString {
        self.clone()
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

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    UnamedVariable,
    UnclosedVariable,
}

#[derive(Error, Debug, PartialEq, Eq)]
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
        } else if variable_start_index.is_some() {
            // Last value is a variable
            return Err(ParseError::UnclosedVariable);
        } else if tokens.is_empty() && !s.is_empty() {
            tokens.push(Token::String(String::from(s)))
        }

        Ok(Template::<'a> { tokens })
    }

    pub fn render<T: Context>(&self, ctx: &T) -> Result<PathBuf, RenderError> {
        let mut result = OsString::default();

        for i in 0..self.tokens.len() {
            let tk = &self.tokens[i];

            match tk {
                Token::String(str) => result.push(&str[..]),
                Token::Variable(name) => {
                    if let Some(value) = ctx.get(name) {
                        result.push(value.render(name, ctx));
                    } else {
                        return Err(RenderError::UndefinedVariable(name.to_string()));
                    }
                }
            }
        }

        Ok(PathBuf::from(result))
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseError, RenderError, Template, TemplateValue};
    use std::{collections::HashMap, path::PathBuf};

    #[test]
    fn string_without_variable() {
        let tpl = Template::parse_str("abcdef").unwrap();
        assert_eq!(tpl.tokens.len(), 1);

        let str = tpl.render(&HashMap::new()).unwrap();
        assert_eq!(str, PathBuf::from("abcdef"));
        let str = tpl.render(&HashMap::new()).unwrap();
        assert_eq!(str, PathBuf::from("abcdef"));

        let mut hmap: HashMap<String, Box<dyn TemplateValue>> = HashMap::new();
        let unused_var = "Hello world".to_owned();
        hmap.insert("k".to_string(), Box::new(unused_var));
        let str = tpl.render(&hmap).unwrap();
        assert_eq!(str, PathBuf::from("abcdef"));
    }

    #[test]
    fn empty_string() {
        let tpl = Template::parse_str("").unwrap();
        assert_eq!(tpl.tokens.len(), 0);

        let str = tpl.render(&HashMap::new()).unwrap();
        assert_eq!(str, PathBuf::from(""));
        let str = tpl.render(&HashMap::new()).unwrap();
        assert_eq!(str, PathBuf::from(""));
    }

    #[test]
    fn string() {
        let tpl =
            Template::parse_str(":date.day:/constant_prefix:date.month:/:date.year:").unwrap();
        assert_eq!(tpl.tokens.len(), 5);

        let mut hmap: HashMap<String, Box<dyn TemplateValue>> = HashMap::new();
        let year = "2022";
        hmap.insert("date.year".to_string(), Box::new(year));
        let month = "08";
        hmap.insert("date.month".to_string(), Box::new(month));
        let day = "19";
        hmap.insert("date.day".to_string(), Box::new(day));

        let str = tpl.render(&hmap).unwrap();
        assert_eq!(str, PathBuf::from("19/constant_prefix08/2022"));

        let str = tpl.render(&hmap).unwrap();
        assert_eq!(str, PathBuf::from("19/constant_prefix08/2022"));
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

pub trait Context {
    fn get(&self, key: &str) -> Option<&Box<dyn TemplateValue>>;
    fn insert(&mut self, key: String, value: Box<dyn TemplateValue>);
}

impl Context for HashMap<String, Box<dyn TemplateValue>> {
    fn get(&self, key: &str) -> Option<&Box<dyn TemplateValue>> {
        self.get(key)
    }

    fn insert(&mut self, key: String, value: Box<dyn TemplateValue>) {
        self.insert(key, value);
    }
}
