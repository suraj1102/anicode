//! Math typesetting with Typst. LaTeX math is translated to Typst first.

mod outline;
mod world;

use anicode_core::kurbo::{Affine, BezPath, Shape as _};
use typst_layout::PagedDocument;

/// Typst points per em in the compiled document. Output is divided by it,
/// so it does not change the result.
const FONT_SIZE_PT: f64 = 10.0;

/// Definitions for the names that converted LaTeX uses.
const MITEX_PRELUDE: &str = include_str!("mitex.typ");

/// Typesets Typst math and returns its outline with y pointing up, in units
/// of the font size (1.0 = one em), centered on its bounding box.
pub fn typst_math(source: &str) -> Result<BezPath, String> {
    typeset("", source)
}

/// Typesets LaTeX math by translating it to Typst. Covers math commands and
/// environments, not packages like TikZ or mhchem.
pub fn latex_math(source: &str) -> Result<BezPath, String> {
    let typst = mitex::convert_math(source, Some(mitex_spec_gen::DEFAULT_SPEC.clone()))
        .map_err(|error| format!("latex: {}", error.trim_start_matches("error: ")))?;
    typeset(MITEX_PRELUDE, &typst)
}

fn typeset(prelude: &str, source: &str) -> Result<BezPath, String> {
    let document = format!(
        "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n\
         #set text(size: {FONT_SIZE_PT}pt)\n\
         {prelude}\n\
         $ {source} $\n"
    );
    let world = world::MathWorld::new(document);
    let compiled: PagedDocument = typst::compile(&world).output.map_err(|errors| {
        let messages: Vec<String> = errors
            .iter()
            .map(|error| error.message.to_string())
            .collect();
        format!("math: {}", messages.join("; "))
    })?;
    let page = compiled.pages().first().ok_or("math: nothing to typeset")?;

    let path = outline::frame_path(&page.frame);
    let center = path.bounding_box().center();
    Ok(
        Affine::scale_non_uniform(1.0 / FONT_SIZE_PT, -1.0 / FONT_SIZE_PT)
            * Affine::translate(-center.to_vec2())
            * path,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typesets_a_fraction() {
        let path = typst_math("x = (a + b) / 2").unwrap();
        let bounds = path.bounding_box();
        assert!(bounds.width() > 2.0, "{bounds:?}");
        // A built-up fraction is taller than one line of text.
        assert!(bounds.height() > 1.5, "{bounds:?}");
        assert!(bounds.center().x.abs() < 1e-9 && bounds.center().y.abs() < 1e-9);
    }

    #[test]
    fn reports_typst_errors() {
        let error = typst_math("x = #nope").unwrap_err();
        assert!(error.starts_with("math: "), "{error}");
    }
}
