use std::error::Error;

use crate::template::context::Context;

mod exif;
mod file;
mod date;

/// Prepares the given template context by adding variables from the following
/// modules:
/// - file
/// - exif
/// - date
pub fn prepare_template_context(ctx: &mut Context) -> Result<(), Box<dyn Error>> {
    file::prepare_template_context(ctx)?;
    exif::prepare_template_context(ctx)?;
    date::prepare_template_context(ctx)?;

    Ok(())
}
