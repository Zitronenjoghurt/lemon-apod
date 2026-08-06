use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use std::sync::LazyLock;

use regex::bytes::Regex;

const SNIFF_LIMIT: usize = 4096;

static CHARSET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)charset\s*=\s*["']?\s*([a-z0-9_\-:]+)"#).expect("static regex is valid")
});

pub fn decode_html(bytes: &[u8]) -> (String, &'static Encoding) {
    let encoding = detect_encoding(bytes);
    let (text, _, _) = encoding.decode(bytes);
    (text.into_owned(), encoding)
}

fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    if let Some((encoding, _)) = Encoding::for_bom(bytes) {
        return encoding;
    }

    let head = &bytes[..bytes.len().min(SNIFF_LIMIT)];
    if let Some(caps) = CHARSET_RE.captures(head)
        && let Some(label) = caps.get(1)
        && let Some(encoding) = Encoding::for_label(label.as_bytes())
    {
        return encoding;
    }

    if std::str::from_utf8(bytes).is_ok() {
        UTF_8
    } else {
        WINDOWS_1252
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honours_a_declared_charset() {
        let mut page =
            b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=iso-8859-1\"><p>caf"
                .to_vec();
        page.push(0xE9); // 'é' in latin-1
        page.extend_from_slice(b"</p>");

        let (text, encoding) = decode_html(&page);
        assert_eq!(encoding, WINDOWS_1252); // encoding_rs maps iso-8859-1 onto windows-1252
        assert!(text.contains("café"), "got {text}");
    }

    #[test]
    fn falls_back_to_windows_1252_for_undeclared_high_bytes() {
        let page = vec![b'<', b'p', b'>', 0xB1, b'<', b'/', b'p', b'>'];
        let (text, encoding) = decode_html(&page);
        assert_eq!(encoding, WINDOWS_1252);
        assert!(text.contains('±'), "got {text}");
    }

    #[test]
    fn keeps_undeclared_utf8_intact() {
        let page = "<p>Messier 31 — 2.5 million ly</p>".as_bytes();
        let (text, encoding) = decode_html(page);
        assert_eq!(encoding, UTF_8);
        assert!(text.contains('—'));
    }
}
