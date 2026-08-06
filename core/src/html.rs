use regex::Regex;
use scraper::{ElementRef, Node};
use url::Url;

const ALLOWED_TAGS: &[&str] = &["a", "b", "i", "em", "strong", "sup", "sub"];
const SKIPPED_TAGS: &[&str] = &["script", "style", "noscript", "iframe", "head"];

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
}

pub fn sanitize(el: ElementRef, base: &Url, opts: &Options<'_>) -> Option<Fragment> {
    let mut walker = Walker {
        out: Fragment::default(),
        base,
        start_after: opts.start_after,
        emitting: opts.start_after.is_none(),
        stop_at: opts.stop_at,
        stopped: false,
        pending_space: false,
    };
    walker.children(el);

    if !walker.emitting {
        return None;
    }

    let mut out = walker.out;
    out.html.truncate(out.html.trim_end().len());
    out.text.truncate(out.text.trim_end().len());
    Some(out)
}

pub fn resolve_url(base: &Url, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }

    let joined = base.join(href).ok()?;
    matches!(joined.scheme(), "http" | "https" | "mailto").then(|| joined.into())
}

struct Walker<'a> {
    out: Fragment,
    base: &'a Url,
    start_after: Option<&'a Regex>,
    emitting: bool,
    stop_at: &'a [&'a str],
    stopped: bool,
    pending_space: bool,
}

impl Walker<'_> {
    fn children(&mut self, el: ElementRef) {
        for child in el.children() {
            if self.stopped {
                break;
            }
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

    fn element(&mut self, el: ElementRef) {
        let name = el.value().name().to_ascii_lowercase();

        if SKIPPED_TAGS.contains(&name.as_str()) {
            return;
        }

        if name == "br" {
            if self.emitting {
                self.out.html.push_str("<br>");
            }
            self.pending_space = true;
            return;
        }

        if matches!(
            name.as_str(),
            "p" | "div"
                | "td"
                | "tr"
                | "table"
                | "center"
                | "li"
                | "ul"
                | "ol"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "blockquote"
                | "hr"
        ) {
            self.pending_space = true;
        }

        let tag = self.emitting.then(|| self.open_tag(&name, el)).flatten();

        if let Some(tag) = &tag {
            self.flush_space();
            self.out.html.push_str(&tag.open);
        }
        self.children(el);
        if let Some(tag) = &tag {
            self.out.html.push_str(&tag.close);
        }
    }

    fn open_tag(&self, name: &str, el: ElementRef) -> Option<Tag> {
        if !ALLOWED_TAGS.contains(&name) {
            return None;
        }

        if name == "a" {
            let href = el.value().attr("href")?;
            let resolved = resolve_url(self.base, href)?;
            return Some(Tag {
                open: format!(r#"<a href="{}">"#, escape_attr(&resolved)),
                close: "</a>".into(),
            });
        }

        Some(Tag {
            open: format!("<{name}>"),
            close: format!("</{name}>"),
        })
    }

    fn text(&mut self, raw: &str) {
        let mut slice = raw;

        if !self.emitting {
            let Some(re) = self.start_after else {
                return;
            };
            let Some(found) = re.find(slice) else {
                return;
            };
            self.emitting = true;
            slice = &slice[found.end()..];
        }

        if !self.stop_at.is_empty() {
            let haystack = slice.to_ascii_lowercase();
            if let Some(cut) = self
                .stop_at
                .iter()
                .filter_map(|needle| haystack.find(needle))
                .min()
            {
                slice = &slice[..cut];
                self.stopped = true;
            }
        }

        self.push_text(slice);
    }

    fn push_text(&mut self, slice: &str) {
        if slice.is_empty() {
            return;
        }

        if slice.starts_with(char::is_whitespace) {
            self.pending_space = true;
        }

        for (index, word) in slice.split_whitespace().enumerate() {
            if index > 0 {
                self.pending_space = true;
            }
            self.flush_space();
            self.out.html.push_str(&escape_text(word));
            self.out.text.push_str(word);
        }

        if slice.ends_with(char::is_whitespace) {
            self.pending_space = true;
        }
    }

    fn flush_space(&mut self) {
        if self.pending_space && !self.out.text.is_empty() {
            self.out.html.push(' ');
            self.out.text.push(' ');
        }
        self.pending_space = false;
    }
}

struct Tag {
    open: String,
    close: String,
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
    }

    #[test]
    fn does_not_invent_whitespace_between_adjacent_nodes() {
        let out = run("<p>anti<b>matter</b></p>", &Options::default()).unwrap();
        assert_eq!(out.text, "antimatter");
        assert_eq!(out.html, "anti<b>matter</b>");
    }
}
