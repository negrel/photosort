use std::error::Error;
use std::path::PathBuf;
use std::result;

use crate::template::context::{Context, Result, TemplateValue};

#[derive(Default)]
struct FileTemplateValue;

impl FileTemplateValue {
    fn filepath(&self, ctx: &Context) -> Result {
        ctx.get_or_err(":file.path")?.render("", ctx)
    }

    fn filepathbuf(&self, ctx: &Context) -> PathBuf {
        PathBuf::from(self.filepath(ctx).unwrap())
    }

    fn filename(&self, ctx: &Context) -> Result {
        let filepath = self.filepathbuf(ctx);

        match filepath.file_name() {
            Some(fname) => Ok(fname.to_owned()),
            None => Ok("".to_owned().into()),
        }
    }

    fn filestem(&self, ctx: &Context) -> Result {
        let filepath = self.filepathbuf(ctx);

        if let Some(fstem) = filepath.file_stem() {
            Ok(fstem.to_owned())
        } else {
            Ok("".to_owned().into())
        }
    }

    fn file_extension(&self, ctx: &Context) -> Result {
        let filepath = self.filepathbuf(ctx);

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
