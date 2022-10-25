use std::{error::Error, result::Result as StdResult};

use thiserror::Error;

use crate::template::context::{Context, Result, TemplateValue};

#[derive(Default)]
struct Date {}

impl Date {
    fn get_one_of(&self, ctx: &Context, keys: &[&str]) -> Result {
        #[derive(Debug, Error)]
        #[error("failed to get or render any of the following variables: {0:?}")]
        struct GetOneOfErr(Vec<String>);

        for key in keys {
            match ctx.get(key) {
                Some(v) => match v.render(key, ctx) {
                    Ok(rendered_value) => return Ok(rendered_value),
                    Err(_) => continue,
                },
                None => continue,
            }
        }

        Err(Box::new(GetOneOfErr(
            Vec::from(keys).iter().map(|k| k.to_string()).collect(),
        )))
    }

    fn date(&self, ctx: &Context) -> Result {
        self.get_one_of(ctx, &["exif.date", "file.md.creation_date"])
    }

    fn date_year(&self, ctx: &Context) -> Result {
        self.get_one_of(ctx, &["exif.date.year", "file.md.creation_date.year"])
    }

    fn date_month(&self, ctx: &Context) -> Result {
        self.get_one_of(ctx, &["exif.date.month", "file.md.creation_date.month"])
    }

    fn date_day(&self, ctx: &Context) -> Result {
        self.get_one_of(ctx, &["exif.date.day", "file.md.creation_date.day"])
    }
}

impl TemplateValue for Date {
    fn render(&self, name: &str, ctx: &Context) -> crate::template::context::Result {
        match name {
            "date" => self.date(ctx),
            "date.year" => self.date_year(ctx),
            "date.month" => self.date_month(ctx),
            "date.day" => self.date_day(ctx),
            _ => unreachable!("unexpected date template variable, please report a bug."),
        }
    }
}

pub fn prepare_template_context(ctx: &mut Context) -> StdResult<(), Box<dyn Error>> {
    ctx.insert(
        &["date", "date.year", "date.month", "date.day"],
        Box::new(Date::default()),
    );

    Ok(())
}
