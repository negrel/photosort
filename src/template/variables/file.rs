use std::error::Error;
use std::path::PathBuf;
use std::result;

use chrono::NaiveDate;
use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error;

use crate::template::context::{Context, Result, TemplateValue};

#[derive(Default)]
struct FileTemplateValue;

lazy_static! {
    static ref DATE_REGEX: Regex =
        Regex::new("[0-9]{4}(-|_)?(0[1-9]|1[0-2])(-|_)?([0-2][1-9]|3[0-1])").unwrap();
}

#[derive(Error, Debug)]
enum FileNameDateError {
    #[error("date not found")]
    DateNotFound,
    #[error("not a valid UTF-8 string")]
    NotUTF8String,
    #[error("failed to parse date: {0}")]
    ParseError(#[from] chrono::ParseError),
}

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

    fn filename_naivedate(&self, ctx: &Context) -> result::Result<NaiveDate, FileNameDateError> {
        let filename = self.filepathbuf(ctx);
        let filename = match filename.to_str() {
            Some(f) => f,
            None => return Err(FileNameDateError::NotUTF8String),
        };

        match DATE_REGEX.find(filename) {
            Some(date_match) => {
                let date_str = date_match.as_str().replace(&['-', '_'][..], "");
                Ok(NaiveDate::parse_from_str(&date_str, "%Y%m%d")?)
            }
            None => Err(FileNameDateError::DateNotFound),
        }
    }

    fn filename_date(&self, ctx: &Context) -> Result {
        let date = self.filename_naivedate(ctx).map_err(Box::new)?;
        Ok(date.format("%Y-%m-%d").to_string().into())
    }

    fn filename_date_year(&self, ctx: &Context) -> Result {
        let date = self.filename_naivedate(ctx).map_err(Box::new)?;
        Ok(date.format("%Y").to_string().into())
    }

    fn filename_date_month(&self, ctx: &Context) -> Result {
        let date = self.filename_naivedate(ctx).map_err(Box::new)?;
        Ok(date.format("%m").to_string().into())
    }

    fn filename_date_day(&self, ctx: &Context) -> Result {
        let date = self.filename_naivedate(ctx).map_err(Box::new)?;
        Ok(date.format("%d").to_string().into())
    }
}

impl TemplateValue for FileTemplateValue {
    fn render(&self, name: &str, ctx: &Context) -> Result {
        match name {
            "file.path" => self.filepath(ctx),
            "file.name" => self.filename(ctx),
            "file.stem" => self.filestem(ctx),
            "file.extension" => self.file_extension(ctx),
            "file.name.date" => self.filename_date(ctx),
            "file.name.date.year" => self.filename_date_year(ctx),
            "file.name.date.month" => self.filename_date_month(ctx),
            "file.name.date.day" => self.filename_date_day(ctx),
            _ => unreachable!("unexpected file template variable, please report a bug."),
        }
    }
}

pub fn prepare_template_context(ctx: &mut Context) -> result::Result<(), Box<dyn Error>> {
    ctx.insert(
        &[
            "file.path",
            "file.name",
            "file.stem",
            "file.extension",
            "file.name.date",
            "file.name.date.year",
            "file.name.date.month",
            "file.name.date.day",
        ],
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

#[cfg(test)]
mod test {
    use super::DATE_REGEX;

    #[test]
    fn test_date_year_regex() {
        assert_eq!(
            DATE_REGEX
                .find("picture-2022-11-01-0000.jpg")
                .unwrap()
                .as_str(),
            "2022-11-01"
        );
        assert_eq!(
            DATE_REGEX
                .find("picture-2022_11-01-0000.jpg")
                .unwrap()
                .as_str(),
            "2022_11-01"
        );
        assert_eq!(
            DATE_REGEX
                .find("picture02022-11-01-0000.jpg")
                .unwrap()
                .as_str(),
            "2022-11-01"
        );

        assert!(DATE_REGEX.find("picture-22-11-01-0000.jpg").is_none());
        assert!(DATE_REGEX.find("picture-022-11-01-0000.jpg").is_none());
    }

    #[test]
    fn test_date_month_regex() {
        assert_eq!(
            DATE_REGEX
                .find("picture-2022-11-01-0000.jpg")
                .unwrap()
                .as_str(),
            "2022-11-01"
        );

        assert!(DATE_REGEX.find("picture-2022-00-01-0000.jpg").is_none());
        assert!(DATE_REGEX.find("picture-2022-13-01-0000.jpg").is_none());
        assert!(DATE_REGEX.find("picture-2022-3-01-0000.jpg").is_none());
    }

    #[test]
    fn test_date_day_regex() {
        assert_eq!(
            DATE_REGEX
                .find("picture-2022-11-01-0000.jpg")
                .unwrap()
                .as_str(),
            "2022-11-01"
        );
        assert_eq!(
            DATE_REGEX
                .find("picture-2022-12-31-0000.jpg")
                .unwrap()
                .as_str(),
            "2022-12-31"
        );

        assert!(DATE_REGEX.find("picture-2022-09-1-0000.jpg").is_none());
        assert!(DATE_REGEX.find("picture-2022-09-00-0000.jpg").is_none());
        assert!(DATE_REGEX.find("picture-2022-09-32-0000.jpg").is_none());
        assert!(DATE_REGEX.find("picture-2022-09-40-0000.jpg").is_none());
    }

    #[test]
    fn test_date_regex() {
        assert_eq!(
            DATE_REGEX
                .find("picture-2022-12-31-0000.jpg")
                .unwrap()
                .as_str(),
            "2022-12-31"
        );
        assert_eq!(
            DATE_REGEX
                .find("picture-2022_12_31-0000.jpg")
                .unwrap()
                .as_str(),
            "2022_12_31"
        );
        assert_eq!(
            DATE_REGEX
                .find("picture-20221231_0000.jpg")
                .unwrap()
                .as_str(),
            "20221231"
        );
        assert_eq!(DATE_REGEX.find("picture-202212310000").unwrap().as_str(), "20221231")
    }
}
