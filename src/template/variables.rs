use std::error::Error;

use crate::template::context::Context;

pub fn prepare_template_context(ctx: &mut Context) -> Result<(), Box<dyn Error>> {
    file::prepare_template_context(ctx)?;

    Ok(())
}

mod file {
    use std::error::Error;
    use std::path::PathBuf;
    use std::{ffi::OsString, fs};

    use crate::template::context::{Context, TemplateValue};

    #[derive(Default)]
    struct FileTemplateValue;

    impl FileTemplateValue {
        fn get_filepath(&self, ctx: &Context) -> PathBuf {
            // get filepath private variables
            let filepath = ctx.get(":file.path").unwrap().render("", ctx);
            PathBuf::from(filepath)
        }

        fn filepath(&self, ctx: &Context) -> OsString {
            let filepath = self.get_filepath(ctx);

            match fs::canonicalize(&filepath) {
                Ok(filepath) => filepath.into(),
                Err(err) => {
                    log::warn!(
                        "failed to canonicalize {:?}, using event path instead: {}",
                        filepath,
                        err
                    );
                    filepath.into_os_string()
                }
            }
        }

        fn filename(&self, ctx: &Context) -> OsString {
            let filepath = self.get_filepath(ctx);

            if let Some(fname) = filepath.file_name() {
                fname.to_owned()
            } else {
                "".to_owned().into()
            }
        }

        fn filestem(&self, ctx: &Context) -> OsString {
            let filepath = self.get_filepath(ctx);

            if let Some(fstem) = filepath.file_stem() {
                fstem.to_owned()
            } else {
                "".to_owned().into()
            }
        }

        fn file_extension(&self, ctx: &Context) -> OsString {
            let filepath = self.get_filepath(ctx);

            // file extension
            if let Some(fext) = filepath.extension() {
                fext.to_owned()
            } else {
                "".to_owned().into()
            }
        }
    }

    impl TemplateValue for FileTemplateValue {
        fn render(&self, name: &str, ctx: &Context) -> std::ffi::OsString {
            match name {
                "file.path" => self.filepath(ctx),
                "file.name" => self.filename(ctx),
                "file.stem" => self.filestem(ctx),
                "file.extension" => self.file_extension(ctx),
                _ => unreachable!("unexpected file template variable, please report a bug."),
            }
        }
    }

    pub fn prepare_template_context(ctx: &mut Context) -> Result<(), Box<dyn Error>> {
        ctx.insert(
            &["file.path", "file.name", "file.stem", "file.extension"],
            Box::new(FileTemplateValue::default()),
        );
        Ok(())
    }
}
