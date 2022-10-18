use std::path::PathBuf;
use std::result;
use std::{error::Error, fs};

use crate::template::context::{Context, Result, TemplateValue};

#[derive(Default)]
struct FileTemplateValue;

impl FileTemplateValue {
    fn get_filepath(&self, ctx: &Context) -> PathBuf {
        // get filepath private variables
        let filepath = ctx.get(":file.path").unwrap().render("", ctx).unwrap();
        PathBuf::from(filepath)
    }

    fn filepath(&self, ctx: &Context) -> Result {
        let filepath = self.get_filepath(ctx);

        match fs::canonicalize(filepath) {
            Ok(filepath) => Ok(filepath.into()),
            Err(err) => Err(Box::new(err)),
        }
    }

    fn filename(&self, ctx: &Context) -> Result {
        let filepath = self.get_filepath(ctx);

        match filepath.file_name() {
            Some(fname) => Ok(fname.to_owned()),
            None => Ok("".to_owned().into()),
        }
    }

    fn filestem(&self, ctx: &Context) -> Result {
        let filepath = self.get_filepath(ctx);

        if let Some(fstem) = filepath.file_stem() {
            Ok(fstem.to_owned())
        } else {
            Ok("".to_owned().into())
        }
    }

    fn file_extension(&self, ctx: &Context) -> Result {
        let filepath = self.get_filepath(ctx);

        // file extension
        if let Some(fext) = filepath.extension() {
            Ok(fext.to_owned())
        } else {
            Ok("".to_owned().into())
        }
    }
}

impl TemplateValue for FileTemplateValue {
    fn render(&self, name: &str, ctx: &Context) -> Result {
        match name {
            "file.path" => self.filepath(ctx),
            "file.name" => self.filename(ctx),
            "file.stem" => self.filestem(ctx),
            "file.extension" => self.file_extension(ctx),
            _ => unreachable!("unexpected file template variable, please report a bug."),
        }
    }
}

pub fn prepare_template_context(ctx: &mut Context) -> result::Result<(), Box<dyn Error>> {
    ctx.insert(
        &["file.path", "file.name", "file.stem", "file.extension"],
        Box::new(FileTemplateValue::default()),
    );
    Ok(())
}
