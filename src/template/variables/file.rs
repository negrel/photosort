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
    metadata::prepare_template_context(ctx)?;

    Ok(())
}

mod metadata {
    use std::{error::Error, fs, io, result::Result as StdResult};

    use chrono::{DateTime, Local};
    use thiserror::Error;

    use crate::template::context::{Context, Result, TemplateValue};

    #[derive(Error, Debug)]
    enum MetadataError {
        #[error("failed to read metadata: {0}")]
        Read(#[from] io::Error),
    }

    #[derive(Default)]
    struct FileMetadataTemplateValue {}

    impl FileMetadataTemplateValue {
        fn creation_datetime(&self, ctx: &Context) -> StdResult<DateTime<Local>, Box<dyn Error>> {
            let filepath = ctx.get_or_err(":file.path")?.render("", ctx)?;

            let md = fs::metadata(filepath).map_err(|e| Box::new(MetadataError::Read(e)))?;
            let systime = md.created()?;

            Ok(DateTime::from(systime))
        }

        fn creation_date(&self, ctx: &Context) -> Result {
            let date = self.creation_datetime(ctx)?;
            Ok(date.format("%Y-%m-%d").to_string().into())
        }

        fn creation_date_year(&self, ctx: &Context) -> Result {
            let date = self.creation_datetime(ctx)?;
            Ok(date.format("%Y").to_string().into())
        }

        fn creation_date_month(&self, ctx: &Context) -> Result {
            let date = self.creation_datetime(ctx)?;
            Ok(date.format("%m").to_string().into())
        }

        fn creation_date_day(&self, ctx: &Context) -> Result {
            let date = self.creation_datetime(ctx)?;
            Ok(date.format("%d").to_string().into())
        }
    }

    impl TemplateValue for FileMetadataTemplateValue {
        fn render(&self, name: &str, ctx: &Context) -> Result {
            match name {
                "file.md.creation_date" => self.creation_date(ctx),
                "file.md.creation_date.year" => self.creation_date_year(ctx),
                "file.md.creation_date.month" => self.creation_date_month(ctx),
                "file.md.creation_date.day" => self.creation_date_day(ctx),
                &_ => {
                    unreachable!("unexpected file metadata template variable, please report a bug.")
                }
            }
        }
    }

    pub fn prepare_template_context(ctx: &mut Context) -> StdResult<(), Box<dyn Error>> {
        ctx.insert(
            &[
                "file.md.creation_date",
                "file.md.creation_date",
                "file.md.creation_date",
                "file.md.creation_date",
            ],
            Box::new(FileMetadataTemplateValue::default()),
        );
        Ok(())
    }
}
