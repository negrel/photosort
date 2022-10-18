use std::error::Error;
use std::path::PathBuf;
use std::result::Result as StdResult;

use exif::{DateTime, Exif, In, Reader, Tag, Value};
use thiserror::Error;

use crate::template::context::{Context, Result, TemplateValue};

struct ExifTemplateValue {
    exif: Exif,
}

#[derive(Error, Debug)]
enum ExifError {
    #[error("failed to retrieve exif field \"{0}\"")]
    MissingField(String),

    #[error("expected field of type \"{0}\", got \"{1:?}\"")]
    WrongType(String, Value),

    #[error("failed to parse exif datetime")]
    ParseDateTime(#[from] exif::Error),
}

impl ExifTemplateValue {
    pub fn new(exif: Exif) -> Self {
        Self { exif }
    }

    fn datetime(&self) -> StdResult<DateTime, ExifError> {
        let ascii = match self.exif.get_field(Tag::DateTime, In::PRIMARY) {
            Some(f) => match &f.value {
                Value::Ascii(ascii) => ascii
                    .iter()
                    .flatten()
                    .map(|v| v.to_owned())
                    .collect::<Vec<u8>>(),
                &_ => return Err(ExifError::WrongType("ascii".to_owned(), f.value.to_owned())),
            },
            None => return Err(ExifError::MissingField(Tag::DateTime.to_string())),
        };

        Ok(DateTime::from_ascii(ascii.as_slice())?)
    }

    fn date(&self) -> Result {
        let date = self.datetime()?;
        // RFC3339
        Ok(format!("{:04}-{:02}-{:02}", date.year, date.month, date.day).into())
    }

    fn date_year(&self) -> Result {
        let date = self.datetime()?;
        Ok(format!("{:04}", date.year).into())
    }

    fn date_month(&self) -> Result {
        let date = self.datetime()?;
        Ok(format!("{:02}", date.month).into())
    }

    fn date_day(&self) -> Result {
        let date = self.datetime()?;
        Ok(format!("{:02}", date.day).into())
    }
}

impl TemplateValue for ExifTemplateValue {
    fn render(&self, name: &str, _ctx: &Context) -> Result {
        match name {
            "exif.date" => self.date(),
            "exif.date.year" => self.date_year(),
            "exif.date.month" => self.date_month(),
            "exif.date.day" => self.date_day(),
            _ => unreachable!("unexpected exif template variable, please report a bug."),
        }
    }
}

pub fn prepare_template_context(ctx: &mut Context) -> StdResult<(), Box<dyn Error>> {
    // get filepath private variables
    let filepath = ctx.get(":file.path").unwrap().render("", ctx)?;
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
