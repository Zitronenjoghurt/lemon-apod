use crate::client::{Client, Limit, Response};
use crate::config::Config;
use crate::video;
use anyhow::{Context, Result};
use apod_core::{ApodDate, ApodWriter, Media, Thumb, ThumbSource};
use std::path::Path;

pub enum Generated {
    Written,
    Adopted,
    NotApplicable,
    Failed(String),
}

impl Generated {
    pub fn fetched(&self) -> bool {
        matches!(self, Self::Written | Self::Failed(_))
    }
}

enum Fetch {
    Still(Vec<String>),
    Frame(String),
}

struct Encoded {
    webp: Vec<u8>,
    width: u32,
    height: u32,
}

pub async fn generate(
    cfg: &Config,
    client: &Client,
    index: &ApodWriter,
    date: ApodDate,
    media: &Media,
    force: bool,
) -> Result<Generated> {
    let source = match media.thumb_source() {
        ThumbSource::None => return Ok(Generated::NotApplicable),
        source => source,
    };

    let on_disk = cfg.thumb_path(date);
    if !force && adoptable(&on_disk) {
        index
            .set_thumb(date, Some(&measured(&date.thumb_path(), &on_disk)))
            .await?;
        return Ok(Generated::Adopted);
    }

    let fetch = match resolve_source(cfg, client, source).await {
        Ok(fetch) => fetch,
        Err(error) => return Ok(Generated::Failed(format!("{error:#}"))),
    };

    let encoded = match fetch {
        Fetch::Still(candidates) => {
            let limit = Limit {
                max_bytes: cfg.thumbs.image_max_bytes,
                timeout: cfg.fetch_timeout,
            };
            match download(client, &candidates, limit).await {
                Err(error) => Err(error),
                Ok(bytes) => encode(&bytes, cfg.thumbs.max_width, cfg.thumbs.quality),
            }
        }
        Fetch::Frame(url) => {
            let limit = Limit {
                max_bytes: cfg.thumbs.video_max_bytes,
                timeout: cfg.thumbs.video_timeout,
            };
            match client.get_limited(&url, limit).await {
                Err(error) => Err(error),
                Ok(Response::NotFound) => Err(anyhow::anyhow!("{url} is gone")),
                Ok(Response::Body(bytes)) => video::poster_frame(&bytes)
                    .with_context(|| format!("taking a frame from {url}"))
                    .and_then(|frame| {
                        encode_frame(&frame, cfg.thumbs.max_width, cfg.thumbs.quality)
                    }),
            }
        }
    };

    match encoded {
        Err(error) => Ok(Generated::Failed(format!("{error:#}"))),
        Ok(encoded) => {
            write(&on_disk, &encoded.webp)?;
            index
                .set_thumb(
                    date,
                    Some(&Thumb::sized(
                        date.thumb_path(),
                        encoded.width,
                        encoded.height,
                    )),
                )
                .await?;
            Ok(Generated::Written)
        }
    }
}

fn adoptable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() > 0)
}

pub fn measured(stored_path: &str, on_disk: &Path) -> Thumb {
    match image::image_dimensions(on_disk) {
        Ok((width, height)) => Thumb::sized(stored_path, width, height),
        Err(error) => {
            tracing::debug!(path = %on_disk.display(), "could not measure the thumbnail: {error}");
            Thumb::new(stored_path)
        }
    }
}

async fn download(client: &Client, candidates: &[String], limit: Limit) -> Result<Vec<u8>> {
    let mut last = None;

    for url in candidates {
        match client.get_limited(url, limit).await {
            Ok(Response::Body(bytes)) => return Ok(bytes),
            Ok(Response::NotFound) => last = Some(anyhow::anyhow!("{url} is gone")),
            Err(error) => last = Some(error),
        }
    }

    Err(last.unwrap_or_else(|| anyhow::anyhow!("nothing to download")))
}

async fn resolve_source(cfg: &Config, client: &Client, source: ThumbSource) -> Result<Fetch> {
    match source {
        ThumbSource::Direct(url) => Ok(Fetch::Still(vec![url])),
        ThumbSource::Frame(url) => Ok(Fetch::Frame(url)),
        ThumbSource::YouTube(id) => Ok(Fetch::Still(
            cfg.thumbs
                .youtube_templates
                .iter()
                .map(|template| template.replace("{id}", &id))
                .collect(),
        )),
        ThumbSource::Vimeo(id) => {
            let endpoint = format!(
                "{}?url=https://vimeo.com/{id}&width={}",
                cfg.thumbs.vimeo_oembed_url, cfg.thumbs.max_width
            );
            let Response::Body(body) = client.get(&endpoint).await? else {
                anyhow::bail!("vimeo has no oembed data for {id}");
            };

            let payload: serde_json::Value =
                serde_json::from_slice(&body).context("parsing the vimeo oembed response")?;
            payload
                .get("thumbnail_url")
                .and_then(|value| value.as_str())
                .map(|url| Fetch::Still(vec![url.to_owned()]))
                .context("vimeo oembed response carried no thumbnail_url")
        }
        ThumbSource::None => anyhow::bail!("nothing to thumbnail"),
    }
}

fn encode(bytes: &[u8], max_width: u32, quality: f32) -> Result<Encoded> {
    let image = image::load_from_memory(bytes).context("decoding the source image")?;
    scale_and_encode(image, max_width, quality)
}

fn encode_frame(frame: &video::Frame, max_width: u32, quality: f32) -> Result<Encoded> {
    let buffer = image::RgbImage::from_raw(frame.width, frame.height, frame.rgb.clone())
        .context("the decoded frame did not fill its own dimensions")?;
    scale_and_encode(image::DynamicImage::ImageRgb8(buffer), max_width, quality)
}

fn scale_and_encode(image: image::DynamicImage, max_width: u32, quality: f32) -> Result<Encoded> {
    let scaled = if image.width() > max_width {
        let height =
            (image.height() as u64 * max_width as u64 / image.width().max(1) as u64).max(1) as u32;
        image.resize_exact(max_width, height, image::imageops::FilterType::Lanczos3)
    } else {
        image
    };

    let (width, height) = (scaled.width(), scaled.height());
    let rgb: image::DynamicImage = scaled.into_rgb8().into();
    let encoder = webp::Encoder::from_image(&rgb)
        .map_err(|error| anyhow::anyhow!("preparing the webp encoder: {error}"))?;

    Ok(Encoded {
        webp: encoder.encode(quality).to_vec(),
        width,
        height,
    })
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbImage};
    use std::io::Cursor;

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, ImageFormat::Jpeg)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn downscales_to_the_configured_width() {
        let encoded = encode(&jpeg(1600, 900), 480, 80.0).unwrap();
        let decoded = image::load_from_memory(&encoded.webp).unwrap();

        assert_eq!(decoded.width(), 480);
        assert_eq!(decoded.height(), 270, "aspect ratio should be preserved");
    }

    #[test]
    fn leaves_images_smaller_than_the_target_alone() {
        let encoded = encode(&jpeg(320, 240), 480, 80.0).unwrap();
        let decoded = image::load_from_memory(&encoded.webp).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (320, 240));
    }

    #[test]
    fn produces_a_small_file() {
        let encoded = encode(&jpeg(1600, 900), 480, 80.0).unwrap();
        assert!(
            encoded.webp.len() < 60_000,
            "thumbnail was {} bytes, expected well under 60KB",
            encoded.webp.len()
        );
    }

    #[test]
    fn reports_the_size_it_actually_wrote() {
        let encoded = encode(&jpeg(1600, 900), 480, 80.0).unwrap();
        assert_eq!((encoded.width, encoded.height), (480, 270));

        let decoded = image::load_from_memory(&encoded.webp).unwrap();
        assert_eq!(
            (encoded.width, encoded.height),
            (decoded.width(), decoded.height())
        );
    }

    #[test]
    fn measures_a_thumbnail_already_on_disk() {
        let dir = std::env::temp_dir().join("apod-measure-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("thumb.webp");
        let encoded = encode(&jpeg(1600, 900), 480, 80.0).unwrap();
        std::fs::write(&path, &encoded.webp).unwrap();

        let thumb = measured("2024/03/2024-03-05.webp", &path);
        assert_eq!(thumb.path, "2024/03/2024-03-05.webp");
        assert_eq!((thumb.width, thumb.height), (Some(480), Some(270)));

        let broken = dir.join("broken.webp");
        std::fs::write(&broken, b"not an image").unwrap();
        let thumb = measured("x.webp", &broken);
        assert_eq!((thumb.width, thumb.height), (None, None));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reports_undecodable_bytes_rather_than_panicking() {
        assert!(encode(b"not an image at all", 480, 80.0).is_err());
    }

    #[test]
    fn adopts_a_thumbnail_already_on_disk_but_not_an_empty_one() {
        let dir = std::env::temp_dir().join("apod-adopt-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        assert!(!adoptable(&dir.join("absent.webp")), "nothing there");
        assert!(!adoptable(&dir), "a directory is not a thumbnail");

        let empty = dir.join("empty.webp");
        std::fs::write(&empty, b"").unwrap();
        assert!(!adoptable(&empty), "half-written files are worth redoing");

        let real = dir.join("real.webp");
        std::fs::write(&real, jpeg(8, 8)).unwrap();
        assert!(adoptable(&real));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_a_network_round_trip_needs_pacing() {
        assert!(Generated::Written.fetched());
        assert!(Generated::Failed("gone".into()).fetched());
        assert!(!Generated::Adopted.fetched());
        assert!(!Generated::NotApplicable.fetched());
    }
}
