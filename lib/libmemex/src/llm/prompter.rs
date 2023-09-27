use std::path::PathBuf;

use handlebars::RenderError;
use serde::Serialize;

pub fn build_template<T>(template_path: PathBuf, data: &T) -> Result<String, RenderError>
where
    T: Serialize,
{
    let mut reg = handlebars::Handlebars::new();
    reg.register_escape_fn(handlebars::no_escape);

    let template = std::fs::read_to_string(template_path).expect("Invalid template path");
    reg.render_template(&template, data)
}
