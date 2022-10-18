use std::error::Error;

use crate::template::context::Context;

mod exif;
mod file;

pub fn prepare_template_context(ctx: &mut Context) -> Result<(), Box<dyn Error>> {
    file::prepare_template_context(ctx)?;
    exif::prepare_template_context(ctx)?;

    Ok(())
}