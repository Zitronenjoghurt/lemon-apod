use crate::client::{Client, Response};
use crate::config::Config;
use crate::index::IndexStore;
use anyhow::{Context, Result};
use apod_core::{ApodDate, Media, ThumbSource};
use std::path::Path;

pub enum Generated {
    Written,
    NotApplicable,
    Failed(String),
}

pub async fn generate(
    cfg: &Config,
    client: &Client,
    index: &mut IndexStore,
    date: ApodDate,
    media: &Media,
) -> Result<Generated> {
    let source = match media.thumb_source() {
        ThumbSource::None => return Ok(Generated::NotApplicable),
        source => source,
    };

    let url = match resolve_source(cfg, client, source).await {
        Ok(url) => url,
        Err(error) => return Ok(Generated::Failed(format!("{error:#}"))),
    };

    let bytes = match client.get(&url).await {
        Ok(Response::Body(bytes)) => bytes,
        Ok(Response::NotFound) => return Ok(Generated::Failed(format!("{url} is gone"))),
        Err(error) => return Ok(Generated::Failed(format!("{error:#}"))),
    };

    let path = cfg.thumb_path(date);
    match encode(&bytes, cfg.thumbs.max_width, cfg.thumbs.quality) {
        Err(error) => Ok(Generated::Failed(format!("{error:#}"))),
        Ok(webp) => {
            write(&path, &webp)?;
            index.set_thumb(date, Some(&date.thumb_path()))?;
            Ok(Generated::Written)
        }
    }
}

async fn resolve_source(cfg: &Config, client: &Client, source: ThumbSource) -> Result<String> {
    match source {
        ThumbSource::Direct(url) => Ok(url),
        ThumbSource::YouTube(id) => Ok(cfg.thumbs.youtube_template.replace("{id}", &id)),
        ThumbSource::Vimeo(id) => {
            let endpoint = format!("{}?url=https://vimeo.com/{id}", cfg.thumbs.vimeo_oembed_url);
            let Response::Body(body) = client.get(&endpoint).await? else {
                anyhow::bail!("vimeo has no oembed data for {id}");
            };

            let payload: serde_json::Value =
                serde_json::from_slice(&body).context("parsing the vimeo oembed response")?;
            payload
                .get("thumbnail_url")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .context("vimeo oembed response carried no thumbnail_url")
        }
        ThumbSource::None => anyhow::bail!("nothing to thumbnail"),
    }
}

fn encode(bytes: &[u8], max_width: u32, quality: f32) -> Result<Vec<u8>> {
    let image = image::load_from_memory(bytes).context("decoding the source image")?;

    let scaled = if image.width() > max_width {
        let height =
            (image.height() as u64 * max_width as u64 / image.width().max(1) as u64).max(1) as u32;
        image.resize_exact(max_width, height, image::imageops::FilterType::Lanczos3)
    } else {
        image
    };

    let rgb: image::DynamicImage = scaled.into_rgb8().into();
    let encoder = webp::Encoder::from_image(&rgb)
        .map_err(|error| anyhow::anyhow!("preparing the webp encoder: {error}"))?;

    Ok(encoder.encode(quality).to_vec())
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
        let decoded = image::load_from_memory(&encoded).unwrap();

        assert_eq!(decoded.width(), 480);
        assert_eq!(decoded.height(), 270, "aspect ratio should be preserved");
    }

    #[test]
    fn leaves_images_smaller_than_the_target_alone() {
        let encoded = encode(&jpeg(320, 240), 480, 80.0).unwrap();
        let decoded = image::load_from_memory(&encoded).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (320, 240));
    }

    #[test]
    fn produces_a_small_file() {
        let encoded = encode(&jpeg(1600, 900), 480, 80.0).unwrap();
        assert!(
            encoded.len() < 60_000,
            "thumbnail was {} bytes, expected well under 60KB",
            encoded.len()
        );
    }

    #[test]
    fn reports_undecodable_bytes_rather_than_panicking() {
        assert!(encode(b"not an image at all", 480, 80.0).is_err());
    }
}
