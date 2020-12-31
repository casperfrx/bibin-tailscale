extern crate syntect;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground};
use syntect::parsing::SyntaxSet;

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

/// Takes the content of a paste and the extension passed in by the viewer and will return the content
/// highlighted in the appropriate format in HTML.
///
/// Returns `None` if the extension isn't supported.
impl Highlighter {
    pub fn highlight(&self, content: &str, ext: &str) -> Option<String> {
        let syntax = self.syntax_set.find_syntax_by_extension(ext)?;
        let mut h = HighlightLines::new(syntax, &self.theme_set.themes["base16-ocean.dark"]);
        let regions = h.highlight(content, &self.syntax_set);

        Some(styled_line_to_highlighted_html(
            &regions[..],
            IncludeBackground::No,
        ))
    }

    pub fn new() -> Highlighter {
        Highlighter {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
}
