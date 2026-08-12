use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    ImageJpg,
    ImagePng,
    ImageGif,
    VideoMp4,
    #[serde(rename = "youtube")]
    YouTube,
    Vimeo,
    Embed,
    Other,
    #[default]
    None,
}

impl MediaKind {
    pub fn is_image(self) -> bool {
        matches!(self, Self::ImageJpg | Self::ImagePng | Self::ImageGif)
    }

    pub fn is_video(self) -> bool {
        matches!(self, Self::VideoMp4 | Self::YouTube | Self::Vimeo)
    }

    pub fn from_url(url: &str) -> Self {
        let path = url.split(['?', '#']).next().unwrap_or(url);
        if let Some(ext) = path
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
        {
            match ext.as_str() {
                "jpg" | "jpeg" => return Self::ImageJpg,
                "png" => return Self::ImagePng,
                "gif" => return Self::ImageGif,
                "mp4" | "m4v" => return Self::VideoMp4,
                _ => {}
            }
        }

        let lower = url.to_ascii_lowercase();
        if lower.contains("youtube.com/") || lower.contains("youtu.be/") {
            Self::YouTube
        } else if lower.contains("vimeo.com/") {
            Self::Vimeo
        } else {
            Self::Other
        }
    }

    pub fn from_embed_url(url: &str) -> Self {
        match Self::from_url(url) {
            Self::Other => Self::Embed,
            kind => kind,
        }
    }
}

impl fmt::Display for MediaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::ImageJpg => "image_jpg",
            Self::ImagePng => "image_png",
            Self::ImageGif => "image_gif",
            Self::VideoMp4 => "video_mp4",
            Self::YouTube => "youtube",
            Self::Vimeo => "vimeo",
            Self::Embed => "embed",
            Self::Other => "other",
            Self::None => "none",
        };
        f.write_str(name)
    }
}

impl FromStr for MediaKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "image_jpg" => Self::ImageJpg,
            "image_png" => Self::ImagePng,
            "image_gif" => Self::ImageGif,
            "video_mp4" => Self::VideoMp4,
            "youtube" => Self::YouTube,
            "vimeo" => Self::Vimeo,
            "embed" => Self::Embed,
            "other" => Self::Other,
            "none" => Self::None,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindFilter(Vec<MediaKind>);

impl KindFilter {
    pub const IMAGE: [MediaKind; 3] = [
        MediaKind::ImageJpg,
        MediaKind::ImagePng,
        MediaKind::ImageGif,
    ];
    pub const VIDEO: [MediaKind; 3] = [MediaKind::VideoMp4, MediaKind::YouTube, MediaKind::Vimeo];

    pub fn new(kinds: impl IntoIterator<Item = MediaKind>) -> Option<Self> {
        let mut unique: Vec<MediaKind> = Vec::new();
        for kind in kinds {
            if !unique.contains(&kind) {
                unique.push(kind);
            }
        }
        (!unique.is_empty()).then_some(Self(unique))
    }

    pub fn kinds(&self) -> &[MediaKind] {
        &self.0
    }
}

impl From<MediaKind> for KindFilter {
    fn from(kind: MediaKind) -> Self {
        Self(vec![kind])
    }
}

impl FromStr for KindFilter {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut kinds = Vec::new();
        for part in s.split(',').map(str::trim).filter(|part| !part.is_empty()) {
            match part {
                "image" | "images" => kinds.extend_from_slice(&Self::IMAGE),
                "video" | "videos" => kinds.extend_from_slice(&Self::VIDEO),
                other => kinds.push(MediaKind::from_str(other)?),
            }
        }
        Self::new(kinds).ok_or(())
    }
}

impl fmt::Display for KindFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, kind) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }
            write!(f, "{kind}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Thumb {
    /// `YYYY/MM/YYYY-MM-DD.webp`, relative to the thumbnail root.
    pub path: String,
    /// Null on thumbnails written before the archiver started recording dimensions.
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl Thumb {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            width: None,
            height: None,
        }
    }

    pub fn sized(path: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            path: path.into(),
            width: Some(width),
            height: Some(height),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThumbSource {
    /// An image to download and scale.
    Direct(String),
    /// A video file to download and take a frame from.
    Frame(String),
    /// A provider video id, whose thumbnail has to be looked up first.
    YouTube(String),
    Vimeo(String),
    None,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Media {
    pub kind: MediaKind,
    pub url: Option<String>,
    pub hd_url: Option<String>,
    /// Where the thumbnail sits under the thumbnail root, as `YYYY/MM/YYYY-MM-DD.webp`.
    /// Readers always fill this in when there is one. It is the storage path and nothing
    /// else, so a consumer can serve it, or open it off disk, without having to unpick
    /// somebody else's URL prefix. Not part of the wire format.
    #[serde(skip)]
    pub thumb_path: Option<String>,
    /// Public URL for the thumbnail. Filled in only when the reader was given a thumbnail
    /// base, which is how the API turns `thumb_path` into `/thumbs/...` for the frontend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb_url: Option<String>,
    /// The thumbnail's pixel size. The thumbnail is the display image scaled down, so this is
    /// also the display image's aspect ratio, which is what lets a client reserve the right
    /// height before either has loaded. Null on entries thumbnailed before this was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb_height: Option<u32>,
}

impl Media {
    pub fn new(kind: MediaKind, url: Option<String>, hd_url: Option<String>) -> Self {
        Self {
            kind,
            url,
            hd_url,
            thumb_path: None,
            thumb_url: None,
            thumb_width: None,
            thumb_height: None,
        }
    }

    pub fn set_thumb(&mut self, thumb: Option<Thumb>, base: Option<&str>) {
        let (path, width, height) = match thumb {
            Some(thumb) => (Some(thumb.path), thumb.width, thumb.height),
            None => (None, None, None),
        };

        self.thumb_url = match (&path, base) {
            (Some(path), Some(base)) => Some(format!("{base}{path}")),
            _ => None,
        };
        self.thumb_path = path;
        self.thumb_width = width;
        self.thumb_height = height;
    }

    pub fn is_empty(&self) -> bool {
        self.url.is_none() && self.hd_url.is_none()
    }

    pub fn best_url(&self) -> Option<&str> {
        self.hd_url.as_deref().or(self.url.as_deref())
    }

    pub fn video_id(&self) -> Option<&str> {
        let url = self.url.as_deref()?;
        let path = url.split(['?', '#', '&']).next().unwrap_or(url);
        let id = match self.kind {
            MediaKind::YouTube => path
                .rsplit_once("/embed/")
                .or_else(|| path.rsplit_once("youtu.be/"))
                .or_else(|| path.rsplit_once("/v/"))
                .map(|(_, id)| id),
            MediaKind::Vimeo => match path.rsplit_once("/video/") {
                Some((_, id)) => Some(id),
                None => url
                    .split(['?', '&'])
                    .skip(1)
                    .find_map(|pair| pair.strip_prefix("clip_id=")),
            },
            _ => None,
        }?;

        let id = id.trim_matches('/');
        (!id.is_empty()).then_some(id)
    }

    pub fn thumb_source(&self) -> ThumbSource {
        match self.kind {
            k if k.is_image() => match self.url.as_deref().or(self.hd_url.as_deref()) {
                Some(url) => ThumbSource::Direct(url.to_owned()),
                None => ThumbSource::None,
            },
            MediaKind::YouTube => match self.video_id() {
                Some(id) => ThumbSource::YouTube(id.to_owned()),
                None => ThumbSource::None,
            },
            MediaKind::Vimeo => match self.video_id() {
                Some(id) => ThumbSource::Vimeo(id.to_owned()),
                None => ThumbSource::None,
            },
            MediaKind::VideoMp4 => match self.url.as_deref() {
                Some(url) => ThumbSource::Frame(url.to_owned()),
                None => ThumbSource::None,
            },
            _ => ThumbSource::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_spells_itself_the_same_way_everywhere() {
        let kinds = [
            MediaKind::ImageJpg,
            MediaKind::ImagePng,
            MediaKind::ImageGif,
            MediaKind::VideoMp4,
            MediaKind::YouTube,
            MediaKind::Vimeo,
            MediaKind::Embed,
            MediaKind::Other,
            MediaKind::None,
        ];

        for kind in kinds {
            let displayed = kind.to_string();
            let serialized = serde_json::to_string(&kind).unwrap();
            assert_eq!(
                serialized,
                format!("\"{displayed}\""),
                "{displayed:?} serializes differently than it displays"
            );
            assert_eq!(MediaKind::from_str(&displayed), Ok(kind));
            assert_eq!(
                serde_json::from_str::<MediaKind>(&serialized).unwrap(),
                kind
            );
        }
    }

    #[test]
    fn classifies_by_extension_and_host() {
        assert_eq!(MediaKind::from_url("a/b/c.JPG"), MediaKind::ImageJpg);
        assert_eq!(MediaKind::from_url("x.png?v=2"), MediaKind::ImagePng);
        assert_eq!(MediaKind::from_url("clip.mp4"), MediaKind::VideoMp4);
        assert_eq!(
            MediaKind::from_url("https://www.youtube.com/embed/abc123"),
            MediaKind::YouTube
        );
        assert_eq!(
            MediaKind::from_url("https://player.vimeo.com/video/98765"),
            MediaKind::Vimeo
        );
        assert_eq!(
            MediaKind::from_url("https://example.com/thing"),
            MediaKind::Other
        );
    }

    #[test]
    fn an_embedded_url_is_never_merely_unknown() {
        assert_eq!(
            MediaKind::from_embed_url("https://stefanom.org/spc/game.php"),
            MediaKind::Embed
        );
        assert_eq!(
            MediaKind::from_embed_url("https://www.youtube.com/embed/abc123"),
            MediaKind::YouTube,
            "an embed the parser does recognise keeps its own kind"
        );
        assert!(!MediaKind::Embed.is_image() && !MediaKind::Embed.is_video());
    }

    #[test]
    fn extracts_provider_video_ids() {
        let yt = Media::new(
            MediaKind::YouTube,
            Some("https://www.youtube.com/embed/dQw4w9WgXcQ?rel=0".into()),
            None,
        );
        assert_eq!(yt.video_id(), Some("dQw4w9WgXcQ"));
        assert_eq!(
            yt.thumb_source(),
            ThumbSource::YouTube("dQw4w9WgXcQ".into())
        );

        let vimeo = Media::new(
            MediaKind::Vimeo,
            Some("https://player.vimeo.com/video/12345678".into()),
            None,
        );
        assert_eq!(vimeo.video_id(), Some("12345678"));
    }

    #[test]
    fn a_flash_era_embed_keeps_its_parameters_out_of_the_id() {
        for url in [
            "https://www.youtube.com/v/zlfKdbWwruY&hl=en_US&fs=1?rel=0&hd=1",
            "https://www.youtube.com/v/zlfKdbWwruY&hl=en_US&fs=1&",
            "https://www.youtube.com/v/zlfKdbWwruY?fs=1&hl=en_US",
            "https://www.youtube.com/v/zlfKdbWwruY",
        ] {
            let media = Media::new(MediaKind::YouTube, Some(url.into()), None);
            assert_eq!(media.video_id(), Some("zlfKdbWwruY"), "from {url}");
        }

        let vimeo = Media::new(
            MediaKind::Vimeo,
            Some("https://www.vimeo.com/moogaloop.swf?clip_id=1250929&server=www.vimeo.com".into()),
            None,
        );
        assert_eq!(vimeo.video_id(), Some("1250929"));
    }

    #[test]
    fn a_self_hosted_video_is_thumbnailed_from_its_own_frames() {
        let media = Media::new(
            MediaKind::VideoMp4,
            Some("https://apod.nasa.gov/apod/image/2607/clip.mp4".into()),
            None,
        );
        assert_eq!(
            media.thumb_source(),
            ThumbSource::Frame("https://apod.nasa.gov/apod/image/2607/clip.mp4".into())
        );
    }

    #[test]
    fn a_kind_filter_expands_the_groups_the_ui_actually_offers() {
        let video: KindFilter = "video".parse().unwrap();
        assert_eq!(video.kinds(), KindFilter::VIDEO);
        assert!(
            video.kinds().contains(&MediaKind::YouTube),
            "the video filter has to reach the embeds, not just the mp4s"
        );

        let images: KindFilter = "image".parse().unwrap();
        assert_eq!(images.kinds(), KindFilter::IMAGE);
    }

    #[test]
    fn a_kind_filter_also_takes_single_kinds_and_lists() {
        assert_eq!(
            "youtube".parse::<KindFilter>().unwrap().kinds(),
            [MediaKind::YouTube]
        );
        assert_eq!(
            "youtube,vimeo".parse::<KindFilter>().unwrap().kinds(),
            [MediaKind::YouTube, MediaKind::Vimeo]
        );
        assert_eq!(
            "video,youtube".parse::<KindFilter>().unwrap().kinds(),
            KindFilter::VIDEO,
            "a kind already covered by a group should not be repeated"
        );

        assert!("mystery".parse::<KindFilter>().is_err());
        assert!("".parse::<KindFilter>().is_err());
        assert!(",,".parse::<KindFilter>().is_err());
    }

    #[test]
    fn a_thumbnail_carries_its_size_when_one_was_recorded() {
        let mut media = Media::new(MediaKind::ImageJpg, None, None);

        media.set_thumb(Some(Thumb::sized("2024/03/x.webp", 480, 320)), Some("/t/"));
        assert_eq!(media.thumb_url.as_deref(), Some("/t/2024/03/x.webp"));
        assert_eq!(
            (media.thumb_width, media.thumb_height),
            (Some(480), Some(320))
        );

        media.set_thumb(Some(Thumb::new("2024/03/x.webp")), Some("/t/"));
        assert_eq!(
            (media.thumb_width, media.thumb_height),
            (None, None),
            "an unmeasured thumbnail must not keep the previous one's size"
        );

        media.set_thumb(None, Some("/t/"));
        assert_eq!(media.thumb_path, None);
        assert_eq!(media.thumb_url, None);
    }

    #[test]
    fn thumbnails_come_from_the_display_image_not_the_hd_one() {
        let media = Media::new(
            MediaKind::ImageJpg,
            Some("https://apod.nasa.gov/apod/image/2503/small.jpg".into()),
            Some("https://apod.nasa.gov/apod/image/2503/huge.jpg".into()),
        );
        assert_eq!(
            media.thumb_source(),
            ThumbSource::Direct("https://apod.nasa.gov/apod/image/2503/small.jpg".into())
        );
    }
}
