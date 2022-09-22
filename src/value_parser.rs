use clap::{builder::TypedValueParser, ErrorKind};

use crate::template::Template;

#[derive(Clone)]
pub struct TemplateParser {}

impl TemplateParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl TypedValueParser for TemplateParser {
    type Value = Template;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let str = match value.to_str() {
            Some(str) => str,
            None => {
                return Err(cmd
                    .clone()
                    .error(ErrorKind::InvalidUtf8, "invalid UTF-8 for template value"))
            }
        };

        match Template::parse_str(str) {
            Ok(tpl) => Ok(tpl),
            Err(err) => Err(cmd.clone().error(ErrorKind::InvalidValue, err.to_string())),
        }
    }
}