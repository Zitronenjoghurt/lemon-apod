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
    YouTube,
    Vimeo,
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
            "other" => Self::Other,
            "none" => Self::None,
            _ => return Err(()),
        })
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb_url: Option<String>,
}

impl Media {
    pub fn new(kind: MediaKind, url: Option<String>, hd_url: Option<String>) -> Self {
        Self {
            kind,
            url,
            hd_url,
            thumb_url: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.url.is_none() && self.hd_url.is_none()
    }

    pub fn best_url(&self) -> Option<&str> {
        self.hd_url.as_deref().or(self.url.as_deref())
    }

    pub fn video_id(&self) -> Option<&str> {
        let url = self.url.as_deref()?;
        let path = url.split(['?', '#']).next().unwrap_or(url);
        let id = match self.kind {
            MediaKind::YouTube => path
                .rsplit_once("/embed/")
                .or_else(|| path.rsplit_once("youtu.be/"))
                .or_else(|| path.rsplit_once("/v/"))
                .map(|(_, id)| id),
            MediaKind::Vimeo => path.rsplit_once("/video/").map(|(_, id)| id),
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
            // Nothing on the page stands in for a self-hosted video: APOD sets no poster
            // attribute, so the only thumbnail available is one decoded from the file itself.
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
