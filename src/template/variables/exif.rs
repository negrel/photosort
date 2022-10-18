use std::path::PathBuf;
use std::result;
use std::{error::Error, ffi::OsString};

use exif::{DateTime, Exif, In, Reader, Tag, Value};

use crate::template::context::{Context, Result, TemplateValue};

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

    fn date(&self) -> Result {
        match self.datetime() {
            // RFC3339
            Some(date) => Ok(format!("{:04}-{:02}-{:02}", date.year, date.month, date.day).into()),
            None => Ok(OsString::default()),
        }
    }

    fn date_year(&self) -> Result {
        match self.datetime() {
            Some(date) => Ok(format!("{:04}", date.year).into()),
            None => Ok(OsString::default()),
        }
    }

    fn date_month(&self) -> Result {
        match self.datetime() {
            Some(date) => Ok(format!("{:02}", date.month).into()),
            None => Ok(OsString::default()),
        }
    }

    fn date_day(&self) -> Result {
        match self.datetime() {
            Some(date) => Ok(format!("{:02}", date.day).into()),
            None => Ok(OsString::default()),
        }
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

pub fn prepare_template_context(ctx: &mut Context) -> result::Result<(), Box<dyn Error>> {
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
