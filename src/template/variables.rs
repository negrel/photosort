use std::error::Error;

use crate::template::context::Context;

pub fn prepare_template_context(ctx: &mut Context) -> Result<(), Box<dyn Error>> {
    file::prepare_template_context(ctx)?;
    exif::prepare_template_context(ctx)?;

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

mod exif {
    use std::path::PathBuf;
    use std::{error::Error, ffi::OsString};

    use exif::{DateTime, Exif, In, Reader, Tag, Value};

    use crate::template::context::{Context, TemplateValue};

    struct ExifTemplateValue {
        exif: Exif,
    }

    impl ExifTemplateValue {
        pub fn new(exif: Exif) -> Self {
            Self { exif }
        }

        fn datetime(&self) -> Option<DateTime> {
            let field = self.exif.get_field(Tag::DateTime, In::PRIMARY);
            let ascii = match field {
                Some(f) => match &f.value {
                    Value::Ascii(ascii) => ascii
                        .iter()
                        .flatten()
                        .map(|v| v.to_owned())
                        .collect::<Vec<u8>>(),
                    &_ => return None,
                },
                None => return None,
            };

            match DateTime::from_ascii(ascii.as_slice()) {
                Ok(datetime) => Some(datetime),
                Err(_err) => None,
            }
        }

        fn date(&self) -> OsString {
            match self.datetime() {
                // RFC3339
                Some(date) => format!("{:04}-{:02}-{:02}", date.year, date.month, date.day).into(),
                None => OsString::default(),
            }
        }

        fn date_year(&self) -> OsString {
            match self.datetime() {
                Some(date) => format!("{:04}", date.year).into(),
                None => OsString::default(),
            }
        }

        fn date_month(&self) -> OsString {
            match self.datetime() {
                Some(date) => format!("{:02}", date.month).into(),
                None => OsString::default(),
            }
        }

        fn date_day(&self) -> OsString {
            match self.datetime() {
                Some(date) => format!("{:02}", date.day).into(),
                None => OsString::default(),
            }
        }
    }

    impl TemplateValue for ExifTemplateValue {
        fn render(&self, name: &str, _ctx: &Context) -> OsString {
            match name {
                "exif.date" => self.date(),
                "exif.date.year" => self.date_year(),
                "exif.date.month" => self.date_month(),
                "exif.date.day" => self.date_day(),
                _ => unreachable!("unexpected exif template variable, please report a bug."),
            }
        }
    }

    pub fn prepare_template_context(ctx: &mut Context) -> Result<(), Box<dyn Error>> {
        // get filepath private variables
        let filepath = ctx.get(":file.path").unwrap().render("", ctx);
        let filepath = PathBuf::from(filepath);

        let file = std::fs::File::open(filepath)?;
        let mut reader = std::io::BufReader::new(&file);

        let exif = match Reader::new().read_from_container(&mut reader) {
            Ok(exif) => exif,
            Err(err) => match err {
                exif::Error::Io(err) => return Err(Box::new(err)),
                _ => return Ok(()),
            },
        };
        let template_value = Box::new(ExifTemplateValue::new(exif));

        ctx.insert(
            &[
                "exif.date",
                "exif.date.year",
                "exif.date.month",
                "exif.date.day",
            ],
            template_value,
        );

        Ok(())
    }
}
