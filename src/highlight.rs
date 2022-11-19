extern crate syntect;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground};
use syntect::parsing::SyntaxSet;
use syntect::Error;

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

/// Takes the content of a paste and the extension passed in by the viewer and will return the content
/// highlighted in the appropriate format in HTML.
///
/// Returns `None` if the extension isn't supported.
impl Highlighter {
    pub fn highlight(&self, content: &str, ext: &str) -> Result<String, Error> {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(ext)
            // Made the decision to always try to return "something", even if the extension is not right.
            // Some extensions might not be recognized by the highlighter but would still be valid. In that
            // case the user will probably still want the extension to be kept in the URL.
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        let mut h = HighlightLines::new(syntax, &self.theme_set.themes["base16-ocean.dark"]);
        let regions = h.highlight_line(content, &self.syntax_set)?;

        styled_line_to_highlighted_html(&regions[..], IncludeBackground::No)
    }

    pub fn new() -> Highlighter {
        Highlighter {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
}
