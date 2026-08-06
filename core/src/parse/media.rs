use crate::html::resolve_url;
use crate::media::{Media, MediaKind};
use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;
use url::Url;

static MEDIA: LazyLock<Selector> = LazyLock::new(|| Selector::parse("img, iframe, video").unwrap());
static ANCHORS: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").unwrap());
static SOURCE: LazyLock<Selector> = LazyLock::new(|| Selector::parse("source[src]").unwrap());

pub fn parse(doc: &Html, base: &Url) -> (Media, Vec<Media>) {
    let mut found: Vec<Media> = doc
        .select(&MEDIA)
        .enumerate()
        .filter_map(|(index, el)| from_element(doc, el, base, index == 0))
        .filter(|media| !media.is_empty())
        .collect();

    if found.is_empty() {
        return (Media::new(MediaKind::None, None, None), Vec::new());
    }

    let extra = found.split_off(1);
    (found.remove(0), extra)
}

fn from_element(doc: &Html, el: ElementRef<'_>, base: &Url, primary: bool) -> Option<Media> {
    match el.value().name() {
        "img" => {
            let url = http_url(base, el.value().attr("src")?)?;
            let hd = hd_link(doc, el, base, primary).filter(|hd| *hd != url);
            Some(Media::new(MediaKind::from_url(&url), Some(url), hd))
        }
        "iframe" => {
            let url = http_url(base, el.value().attr("src")?)?;
            Some(Media::new(MediaKind::from_url(&url), Some(url), None))
        }
        "video" => {
            let src = el.value().attr("src").map(str::to_owned).or_else(|| {
                el.select(&SOURCE)
                    .next()
                    .and_then(|s| s.value().attr("src").map(str::to_owned))
            })?;
            let url = http_url(base, &src)?;
            Some(Media::new(MediaKind::from_url(&url), Some(url), None))
        }
        _ => None,
    }
}

fn hd_link(doc: &Html, img: ElementRef<'_>, base: &Url, primary: bool) -> Option<String> {
    for ancestor in img.ancestors() {
        let Some(el) = ElementRef::wrap(ancestor) else {
            continue;
        };
        if el.value().name() == "a"
            && let Some(href) = el.value().attr("href")
            && let Some(url) = http_url(base, href)
            && is_image_link(&url)
        {
            return Some(url);
        }
    }

    if !primary {
        return None;
    }

    doc.select(&ANCHORS)
        .filter_map(|a| http_url(base, a.value().attr("href")?))
        .find(|url| is_image_link(url))
}

fn is_image_link(url: &str) -> bool {
    MediaKind::from_url(url).is_image() && url.contains("/image/")
}

fn http_url(base: &Url, href: &str) -> Option<String> {
    let resolved = resolve_url(base, href)?;
    resolved.starts_with("http").then_some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://apod.nasa.gov/apod/ap240305.html").unwrap()
    }

    #[test]
    fn resolves_the_image_and_its_full_resolution_link() {
        let doc = Html::parse_document(
            r#"<body><a href="image/2403/big.jpg"><img src="image/2403/small.jpg"></a></body>"#,
        );

        let (media, extra) = parse(&doc, &base());
        assert_eq!(media.kind, MediaKind::ImageJpg);
        assert_eq!(
            media.url.as_deref(),
            Some("https://apod.nasa.gov/apod/image/2403/small.jpg")
        );
        assert_eq!(
            media.hd_url.as_deref(),
            Some("https://apod.nasa.gov/apod/image/2403/big.jpg")
        );
        assert!(extra.is_empty());
    }

    #[test]
    fn gives_each_image_on_a_multi_image_page_its_own_hd_link() {
        let doc = Html::parse_document(
            r#"<body>
               <a href="image/2403/one_big.jpg"><img src="image/2403/one.jpg"></a>
               <a href="image/2403/two_big.jpg"><img src="image/2403/two.jpg"></a>
               </body>"#,
        );

        let (media, extra) = parse(&doc, &base());
        assert_eq!(
            media.hd_url.as_deref(),
            Some("https://apod.nasa.gov/apod/image/2403/one_big.jpg")
        );
        assert_eq!(extra.len(), 1);
        assert_eq!(
            extra[0].hd_url.as_deref(),
            Some("https://apod.nasa.gov/apod/image/2403/two_big.jpg")
        );
    }

    #[test]
    fn an_hd_link_identical_to_the_image_is_dropped() {
        let doc = Html::parse_document(
            r#"<body><a href="image/2608/spokes.gif"><img src="image/2608/spokes.gif"></a></body>"#,
        );

        let (media, _) = parse(&doc, &base());
        assert_eq!(
            media.url.as_deref(),
            Some("https://apod.nasa.gov/apod/image/2608/spokes.gif")
        );
        assert_eq!(media.hd_url, None);
    }

    #[test]
    fn later_images_do_not_inherit_the_first_images_hd_link() {
        let doc = Html::parse_document(
            r#"<body>
               <a href="image/2403/one_big.jpg"><img src="image/2403/one.jpg"></a>
               <img src="image/2403/two.jpg">
               </body>"#,
        );

        let (_, extra) = parse(&doc, &base());
        assert_eq!(extra.len(), 1);
        assert_eq!(extra[0].hd_url, None);
    }

    #[test]
    fn picks_up_embedded_video() {
        let doc = Html::parse_document(
            r#"<body><iframe src="https://www.youtube.com/embed/abc123?rel=0"></iframe></body>"#,
        );

        let (media, _) = parse(&doc, &base());
        assert_eq!(media.kind, MediaKind::YouTube);
        assert_eq!(media.video_id(), Some("abc123"));
    }

    #[test]
    fn picks_up_self_hosted_mp4() {
        let doc = Html::parse_document(
            r#"<body><video><source src="image/2403/clip.mp4"></video></body>"#,
        );

        let (media, _) = parse(&doc, &base());
        assert_eq!(media.kind, MediaKind::VideoMp4);
        assert_eq!(
            media.url.as_deref(),
            Some("https://apod.nasa.gov/apod/image/2403/clip.mp4")
        );
    }

    #[test]
    fn reports_none_when_a_page_has_no_media() {
        let doc = Html::parse_document("<body><p>text only</p></body>");
        let (media, extra) = parse(&doc, &base());
        assert_eq!(media.kind, MediaKind::None);
        assert!(media.is_empty());
        assert!(extra.is_empty());
    }
}
