use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;
use std::str::FromStr;
use std::{fs, io};

use thiserror::Error;

use super::variables;

/// Context define the rendering context of a Template. It contains template value.
#[derive(Default)]
pub struct Context {
    variables: HashMap<String, usize>,
    template_values: Vec<Box<dyn TemplateValue>>,
}

impl Context {
    pub fn get(&self, key: &str) -> Option<&dyn TemplateValue> {
        let index = match self.variables.get(&key.to_string()) {
            Some(index) => index,
            None => return None,
        };

        self.template_values
            .get(index.to_owned())
            .map(|v| v.as_ref())
    }

    pub fn get_or_err(&self, key: &str) -> StdResult<&dyn TemplateValue, Box<dyn Error>> {
        self.get(key)
            .ok_or_else(|| missing_variable(key.to_string()))
    }

    pub fn insert(&mut self, keys: &[&str], value: Box<dyn TemplateValue>) {
        assert!(!keys.is_empty());

        let index = self.template_values.len();
        self.template_values.push(value);

        for key in keys {
            self.variables.insert(key.to_string(), index);
        }
    }
}

#[derive(Error, Debug)]
enum PrivateVariableError {
    #[error("failed to canonicalize filepath: {0}")]
    AbsoluteFilePath(#[from] io::Error),
}

pub fn prepare_template_context(ctx: &mut Context, path: &Path) -> StdResult<(), Box<dyn Error>> {
    let abs_path = match fs::canonicalize(path) {
        Ok(path) => path,
        Err(err) => return Err(Box::new(PrivateVariableError::AbsoluteFilePath(err))),
    };

    // Private variables starts with a ":"
    // :file.path is one of the most important private variable, it used
    // by other template value to fetch absolute filepath.
    ctx.insert(&[":file.path"], Box::new(abs_path));

    variables::prepare_template_context(ctx)?;

    Ok(())
}

pub fn missing_variable(name: String) -> Box<dyn Error> {
    #[derive(Error, Debug)]
    #[error("missing variable \"{0}\"")]
    struct MissingVariableError(String);

    Box::new(MissingVariableError(name))
}

pub type Result = StdResult<OsString, Box<dyn Error>>;

/// TemplateValue defines a value used in the rendering of a [`Template`].
/// It should be stateless and reusable.
/// [`render()`] takes a `name` parameter because a [`TemplateValue`]
/// can be stored multiple times in a [`Context`] with different keys.
pub trait TemplateValue {
    fn render(&self, name: &str, ctx: &Context) -> Result;
}

impl TemplateValue for dyn ToString {
    fn render(&self, name: &str, ctx: &Context) -> Result {
        self.to_string().render(name, ctx)
    }
}

impl TemplateValue for &str {
    fn render(&self, name: &str, ctx: &Context) -> Result {
        self.to_owned().to_owned().render(name, ctx)
    }
}

impl TemplateValue for String {
    fn render(&self, _name: &str, _ctx: &Context) -> Result {
        Ok(OsString::from_str(self).unwrap())
    }
}

impl TemplateValue for PathBuf {
    fn render(&self, _name: &str, _ctx: &Context) -> Result {
        Ok(self.clone().into_os_string())
    }
}

impl TemplateValue for OsString {
    fn render(&self, _name: &str, _ctx: &Context) -> Result {
        Ok(self.clone())
    }
}
