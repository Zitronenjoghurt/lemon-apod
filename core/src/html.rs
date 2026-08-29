use regex::Regex;
use scraper::{ElementRef, Node};
use std::ops::Range;
use std::rc::Rc;
use url::Url;

const ALLOWED_TAGS: &[&str] = &["a", "b", "i", "em", "strong", "sup", "sub"];
const SKIPPED_TAGS: &[&str] = &["script", "style", "noscript", "iframe", "head"];

const BLOCK_TAGS: &[&str] = &[
    "p",
    "div",
    "td",
    "tr",
    "table",
    "center",
    "li",
    "ul",
    "ol",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "blockquote",
    "hr",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fragment {
    pub html: String,
    pub text: String,
}

impl Fragment {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[derive(Default)]
pub struct Options<'a> {
    pub start_after: Option<&'a Regex>,
    pub stop_at: &'a [&'a str],
    /// End the window where this many line breaks run together, which is how a source that puts
    /// its whole page body in one paragraph separates the prose from what follows it.
    pub stop_after_breaks: Option<usize>,
}

/// Extract an element's inline content as sanitized HTML plus the same content as plain text.
pub fn sanitize(el: ElementRef<'_>, base: &Url, opts: &Options<'_>) -> Option<Fragment> {
    let flat = flatten(el, base);
    let window = flat.window(opts)?;
    Some(flat.slice(window))
}

pub fn escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&#39;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

pub fn collapse(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn resolve_url(base: &Url, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }

    let joined = base.join(href).ok()?;
    matches!(joined.scheme(), "http" | "https" | "mailto").then(|| joined.into())
}

/// An element flattened into whitespace-normalised text plus enough structure to rebuild
/// well-formed inline HTML for any range of that text.
///
/// Two passes rather than one streaming pass is what lets a marker span element boundaries.
/// APOD writes its credit label as `Image Credit & <a href="...">Copyright</a>:`, and no match
/// against a single text node can ever see that whole.
pub struct Flat {
    text: String,
    pieces: Vec<Piece>,
}

/// One run of text, carrying the inline tags that were open around it.
struct Piece {
    /// Byte range within [`Flat::text`]. Always on character boundaries.
    range: Range<usize>,
    stack: Vec<Rc<Tag>>,
    breaks: usize,
}

struct Tag {
    open: String,
    close: String,
}

pub fn flatten(el: ElementRef<'_>, base: &Url) -> Flat {
    let mut builder = Builder {
        base,
        text: String::new(),
        pieces: Vec::new(),
        stack: Vec::new(),
        pending_space: false,
        pending_breaks: 0,
    };
    builder.children(el);

    Flat {
        text: builder.text,
        pieces: builder.pieces,
    }
}

impl Flat {
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The range [`Options`] selects: everything after `start_after`, up to the first `stop_at`
    /// or paragraph break, whichever comes first.
    pub fn window(&self, opts: &Options<'_>) -> Option<Range<usize>> {
        let start = match opts.start_after {
            Some(marker) => marker.find(&self.text)?.end(),
            None => 0,
        };

        let mut stop = self.stop(start, opts.stop_at);
        if let Some(breaks) = opts.stop_after_breaks {
            stop = stop.min(self.paragraph_break(start, breaks));
        }
        Some(start..stop)
    }

    /// Where the first run of at least `breaks` line breaks begins after `from`, or the end of
    /// the text. A break at the very start belongs to the marker, not to a paragraph.
    fn paragraph_break(&self, from: usize, breaks: usize) -> usize {
        self.pieces
            .iter()
            .find(|piece| piece.breaks >= breaks && piece.range.start > from)
            .map_or(self.text.len(), |piece| piece.range.start)
    }

    /// Where the first `stop_at` needle appears at or after `from`, or the end of the text.
    ///
    /// The needles are lowercase ASCII, and ASCII case folding never changes a string's length,
    /// so an index into the folded copy is also an index into the original.
    pub fn stop(&self, from: usize, stop_at: &[&str]) -> usize {
        if stop_at.is_empty() {
            return self.text.len();
        }

        let haystack = self.text[from..].to_ascii_lowercase();
        stop_at
            .iter()
            .filter_map(|needle| haystack.find(needle))
            .min()
            .map_or(self.text.len(), |hit| from + hit)
    }

    /// Rebuild a range of the text as a [`Fragment`], trimmed to whole words at both ends.
    pub fn slice(&self, range: Range<usize>) -> Fragment {
        let range = self.trim(range);

        let mut out = Fragment::default();
        let mut open: Vec<&Rc<Tag>> = Vec::new();
        let mut previous_end: Option<usize> = None;

        for piece in &self.pieces {
            let start = piece.range.start.max(range.start);
            let end = piece.range.end.min(range.end);
            if start >= end {
                continue;
            }

            // Close, then separate, then open: a separator that lands between two different
            // tag runs belongs to neither of them.
            let common = common_prefix(&open, &piece.stack);
            for tag in open.drain(common..).rev() {
                out.html.push_str(&tag.close);
            }

            if let Some(previous) = previous_end {
                if piece.breaks > 0 {
                    out.html.push_str("<br>");
                }
                if previous < start {
                    out.html.push(' ');
                    out.text.push(' ');
                }
            }

            for tag in &piece.stack[common..] {
                out.html.push_str(&tag.open);
                open.push(tag);
            }

            let slice = &self.text[start..end];
            out.html.push_str(&escape_text(slice));
            out.text.push_str(slice);
            previous_end = Some(end);
        }

        for tag in open.iter().rev() {
            out.html.push_str(&tag.close);
        }
        out
    }

    /// Shrink a range to the first and last non-whitespace byte inside it.
    fn trim(&self, range: Range<usize>) -> Range<usize> {
        let slice = &self.text[range.clone()];
        let start = range.start + (slice.len() - slice.trim_start().len());
        let end = range.end - (slice.len() - slice.trim_end().len());
        start..end.max(start)
    }
}

fn common_prefix(open: &[&Rc<Tag>], stack: &[Rc<Tag>]) -> usize {
    open.iter()
        .zip(stack)
        .take_while(|(a, b)| Rc::ptr_eq(**a, b))
        .count()
}

struct Builder<'a> {
    base: &'a Url,
    text: String,
    pieces: Vec<Piece>,
    stack: Vec<Rc<Tag>>,
    pending_space: bool,
    pending_breaks: usize,
}

impl Builder<'_> {
    fn children(&mut self, el: ElementRef<'_>) {
        for child in el.children() {
            match child.value() {
                Node::Text(text) => self.text(text),
                Node::Element(_) => {
                    if let Some(child_el) = ElementRef::wrap(child) {
                        self.element(child_el);
                    }
                }
                _ => {}
            }
        }
    }

    fn element(&mut self, el: ElementRef<'_>) {
        let name = el.value().name().to_ascii_lowercase();

        if SKIPPED_TAGS.contains(&name.as_str()) {
            return;
        }

        if name == "br" {
            self.pending_breaks += 1;
            self.pending_space = true;
            return;
        }

        if BLOCK_TAGS.contains(&name.as_str()) {
            self.pending_space = true;
        }

        let tag = self.tag(&name, el);
        if let Some(tag) = &tag {
            self.stack.push(Rc::clone(tag));
        }
        self.children(el);
        if tag.is_some() {
            self.stack.pop();
        }
    }

    fn tag(&self, name: &str, el: ElementRef<'_>) -> Option<Rc<Tag>> {
        if !ALLOWED_TAGS.contains(&name) {
            return None;
        }

        if name == "a" {
            let href = el.value().attr("href")?;
            let resolved = resolve_url(self.base, href)?;
            return Some(Rc::new(Tag {
                open: format!(r#"<a href="{}">"#, escape_attr(&resolved)),
                close: "</a>".into(),
            }));
        }

        Some(Rc::new(Tag {
            open: format!("<{name}>"),
            close: format!("</{name}>"),
        }))
    }

    fn text(&mut self, raw: &str) {
        if raw.is_empty() {
            return;
        }
        if raw.trim().is_empty() {
            self.pending_space = true;
            return;
        }

        if raw.starts_with(char::is_whitespace) {
            self.pending_space = true;
        }
        if self.pending_space && !self.text.is_empty() {
            self.text.push(' ');
        }
        self.pending_space = raw.ends_with(char::is_whitespace);

        let start = self.text.len();
        for (index, word) in raw.split_whitespace().enumerate() {
            if index > 0 {
                self.text.push(' ');
            }
            self.text.push_str(word);
        }

        self.pieces.push(Piece {
            range: start..self.text.len(),
            stack: self.stack.clone(),
            breaks: std::mem::take(&mut self.pending_breaks),
        });
    }
}

fn escape_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_attr(raw: &str) -> String {
    escape_text(raw).replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::{Html, Selector};
    use std::sync::LazyLock;

    static BODY: LazyLock<Selector> = LazyLock::new(|| Selector::parse("body").unwrap());

    fn base() -> Url {
        Url::parse("https://apod.nasa.gov/apod/ap240305.html").unwrap()
    }

    fn run(html: &str, opts: &Options<'_>) -> Option<Fragment> {
        let doc = Html::parse_document(html);
        let body = doc.select(&BODY).next().unwrap();
        sanitize(body, &base(), opts)
    }

    #[test]
    fn keeps_allowed_tags_and_absolutises_links() {
        let out = run(
            r#"<p>See <a href="ap240304.html">yesterday</a> and <b>note</b>.</p>"#,
            &Options::default(),
        )
        .unwrap();

        assert_eq!(
            out.html,
            r#"See <a href="https://apod.nasa.gov/apod/ap240304.html">yesterday</a> and <b>note</b>."#
        );
        assert_eq!(out.text, "See yesterday and note.");
    }

    #[test]
    fn unwraps_disallowed_tags_but_keeps_their_text() {
        let out = run(
            r#"<p><font color="red">red</font> <span>text</span></p>"#,
            &Options::default(),
        )
        .unwrap();
        assert_eq!(out.html, "red text");
    }

    #[test]
    fn drops_scripts_entirely() {
        let out = run(
            "<p>before <script>alert(1)</script> after</p>",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(out.text, "before after");
        assert!(!out.html.contains("alert"));
    }

    #[test]
    fn escapes_text_so_markup_cannot_be_injected() {
        let out = run("<p>5 &lt; 7 &amp; 8 &gt; 2</p>", &Options::default()).unwrap();
        assert_eq!(out.html, "5 &lt; 7 &amp; 8 &gt; 2");
        assert_eq!(out.text, "5 < 7 & 8 > 2");
    }

    #[test]
    fn drops_non_http_schemes() {
        let out = run(
            r#"<p><a href="javascript:alert(1)">click</a></p>"#,
            &Options::default(),
        )
        .unwrap();
        assert_eq!(out.html, "click");
    }

    #[test]
    fn starts_after_a_marker_and_stops_at_a_delimiter() {
        let marker = Regex::new(r"(?i)explanation:\s*").unwrap();
        let opts = Options {
            start_after: Some(&marker),
            stop_at: &["tomorrow's picture"],
            stop_after_breaks: None,
        };
        let out = run(
            "<p><b>Explanation:</b> The <a href='x.html'>galaxy</a> is big. \
             Tomorrow's picture: something else</p>",
            &opts,
        )
        .unwrap();

        assert_eq!(out.text, "The galaxy is big.");
        assert!(out.html.starts_with("The <a href="));
    }

    #[test]
    fn returns_none_when_the_marker_never_appears() {
        let marker = Regex::new("Explanation:").unwrap();
        let opts = Options {
            start_after: Some(&marker),
            stop_at: &[],
            stop_after_breaks: None,
        };
        assert!(run("<p>no marker here</p>", &opts).is_none());
    }

    #[test]
    fn collapses_whitespace_across_tag_boundaries() {
        let out = run(
            "<p>one\n\n  two <b>three</b>\tfour  </p>",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(out.text, "one two three four");
        assert_eq!(out.html, "one two <b>three</b> four");
    }

    #[test]
    fn does_not_invent_whitespace_between_adjacent_nodes() {
        let out = run("<p>anti<b>matter</b></p>", &Options::default()).unwrap();
        assert_eq!(out.text, "antimatter");
        assert_eq!(out.html, "anti<b>matter</b>");
    }

    #[test]
    fn a_marker_may_span_element_boundaries() {
        let marker = Regex::new(r"(?i)\bcredit\s*&\s*copyright\s*:\s*").unwrap();
        let opts = Options {
            start_after: Some(&marker),
            stop_at: &[],
            stop_after_breaks: None,
        };
        let out = run(
            r#"<center><b>Image Credit &amp;
               <a href="lib/about_apod.html">Copyright</a>:</b>
               <a href="https://example.com/">Jane Doe</a></center>"#,
            &opts,
        )
        .unwrap();

        assert_eq!(out.text, "Jane Doe");
        assert_eq!(out.html, r#"<a href="https://example.com/">Jane Doe</a>"#);
    }

    #[test]
    fn a_stop_phrase_may_span_element_boundaries() {
        let opts = Options {
            start_after: None,
            stop_at: &["tomorrow's picture"],
            stop_after_breaks: None,
        };
        let out = run(
            "<p>The prose. <b>Tomorrow's</b> picture: something else</p>",
            &opts,
        )
        .unwrap();
        assert_eq!(out.text, "The prose.");
    }

    #[test]
    fn a_link_broken_over_several_text_nodes_stays_one_link() {
        let out = run(
            r#"<p><a href="https://example.com/">Jane <b>Q</b> Doe</a></p>"#,
            &Options::default(),
        )
        .unwrap();
        assert_eq!(
            out.html,
            r#"<a href="https://example.com/">Jane <b>Q</b> Doe</a>"#
        );
    }

    #[test]
    fn a_separator_between_two_tag_runs_belongs_to_neither() {
        let out = run("<p><b>one</b> <i>two</i></p>", &Options::default()).unwrap();
        assert_eq!(out.html, "<b>one</b> <i>two</i>");
    }

    #[test]
    fn a_run_of_breaks_ends_the_window_but_a_single_one_does_not() {
        let opts = |breaks| Options {
            start_after: None,
            stop_at: &[],
            stop_after_breaks: breaks,
        };

        let one_line = "<p>The prose<br>runs on.<br><br>An announcement below it.</p>";
        assert_eq!(
            run(one_line, &opts(Some(2))).unwrap().text,
            "The prose runs on.",
            "a source that puts its whole body in one paragraph separates it with a blank line"
        );
        assert_eq!(
            run(one_line, &opts(None)).unwrap().text,
            "The prose runs on. An announcement below it.",
            "and a source that does not ask for the boundary keeps everything"
        );
    }

    #[test]
    fn line_breaks_survive_as_breaks() {
        let out = run("<p>one<br>two</p>", &Options::default()).unwrap();
        assert_eq!(out.text, "one two");
        assert_eq!(out.html, "one<br> two");
    }

    #[test]
    fn any_range_of_the_flattened_text_can_be_sliced_back_out() {
        let doc = Html::parse_document("<body><p>Alpha <b>beta</b> gamma</p></body>");
        let body = doc.select(&BODY).next().unwrap();
        let flat = flatten(body, &base());

        assert_eq!(flat.text(), "Alpha beta gamma");
        assert_eq!(flat.slice(6..10).text, "beta");
        assert_eq!(flat.slice(6..10).html, "<b>beta</b>");
        assert_eq!(flat.slice(0..11).html, "Alpha <b>beta</b>");
    }
}
