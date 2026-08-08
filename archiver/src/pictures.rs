use crate::config::Config;
use anyhow::{Context, Result};
use apod_core::apod::pictures::{PHASH_BYTES, PictureGroup};
use apod_core::{ApodDate, ApodWriter};
use std::path::Path;

const ROWS: u32 = 16;
const COLUMNS: u32 = ROWS + 1;

pub struct Report {
    pub hashed: usize,
    pub failed: usize,
    pub groups: Vec<PictureGroup>,
}

impl Report {
    pub fn entries(&self) -> usize {
        self.groups.iter().map(|group| group.dates.len()).sum()
    }
}

pub fn phash(path: &Path) -> Result<Vec<u8>> {
    let image = image::open(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(bits(&cells(&image)))
}

fn cells(image: &image::DynamicImage) -> Vec<f32> {
    let gray = image.to_luma32f();
    let (width, height) = (gray.width().max(1), gray.height().max(1));

    let mut out = Vec::with_capacity((COLUMNS * ROWS) as usize);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let left = column * width / COLUMNS;
            let right = ((column + 1) * width / COLUMNS).max(left + 1).min(width);
            let top = row * height / ROWS;
            let bottom = ((row + 1) * height / ROWS).max(top + 1).min(height);

            let mut total = 0.0f32;
            let mut counted = 0.0f32;
            for y in top..bottom {
                for x in left..right {
                    total += gray.get_pixel(x.min(width - 1), y.min(height - 1)).0[0];
                    counted += 1.0;
                }
            }
            out.push(total / counted.max(1.0));
        }
    }

    out
}

fn bits(cells: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(PHASH_BYTES);
    let mut byte = 0u8;
    let mut filled = 0;

    for row in 0..ROWS as usize {
        for column in 0..ROWS as usize {
            let at = row * COLUMNS as usize + column;
            let brighter = cells[at] > cells[at + 1];
            byte = (byte << 1) | u8::from(brighter);
            filled += 1;
            if filled == 8 {
                bytes.push(byte);
                (byte, filled) = (0, 0);
            }
        }
    }

    debug_assert_eq!(bytes.len(), PHASH_BYTES);
    bytes
}

pub async fn store(cfg: &Config, index: &ApodWriter, date: ApodDate) -> Result<bool> {
    let path = cfg.thumb_path(date);
    if !path.is_file() {
        return Ok(false);
    }

    let phash = phash(&path)?;
    index.set_phash(date, Some(&phash)).await?;
    Ok(true)
}

pub async fn refresh(cfg: &Config, index: &ApodWriter, force: bool) -> Result<Report> {
    let pending = if force {
        index.stored_thumbs().await?
    } else {
        index.unhashed_thumbs().await?
    };

    let (mut hashed, mut failed) = (0, 0);
    for (date, stored_path) in pending {
        match phash(&cfg.thumb_dir.join(&stored_path)) {
            Ok(phash) => {
                index.set_phash(date, Some(&phash)).await?;
                hashed += 1;
            }
            Err(error) => {
                failed += 1;
                tracing::warn!(%date, "could not hash the thumbnail: {error:#}");
            }
        }
    }

    Ok(Report {
        hashed,
        failed,
        groups: index.regroup_pictures().await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use apod_core::apod::pictures::{MAX_DISTANCE, alike, distance};
    use image::{ImageFormat, RgbImage};

    fn write(dir: &Path, name: &str, image: RgbImage) -> std::path::PathBuf {
        let path = dir.join(name);
        image
            .save_with_format(&path, ImageFormat::WebP)
            .expect("writing a test webp");
        path
    }

    fn gradient(width: u32, height: u32, shape: Shape) -> RgbImage {
        let (across_turns, down_turns, phase) = shape.wave();
        RgbImage::from_fn(width, height, |x, y| {
            let across = x as f32 / width as f32 * across_turns + phase;
            let down = y as f32 / height as f32 * down_turns;
            let value = ((across.sin() * down.cos() + 1.0) * 127.0) as u8;
            image::Rgb([value, value.wrapping_add(40), 255 - value])
        })
    }

    #[derive(Clone, Copy)]
    enum Shape {
        One,
        Another,
    }

    impl Shape {
        fn wave(self) -> (f32, f32, f32) {
            match self {
                Self::One => (5.0, 3.0, 0.0),
                Self::Another => (2.0, 7.0, 1.9),
            }
        }
    }

    fn dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_hash_is_always_the_full_width() {
        let dir = dir("apod-phash-width");
        let path = write(&dir, "a.webp", gradient(480, 271, Shape::One));
        assert_eq!(phash(&path).unwrap().len(), PHASH_BYTES);

        let tiny = write(&dir, "b.webp", gradient(3, 2, Shape::One));
        assert_eq!(
            phash(&tiny).unwrap().len(),
            PHASH_BYTES,
            "an image smaller than the hash still has to fill it"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_same_picture_at_another_size_hashes_alike() {
        let dir = dir("apod-phash-scale");

        let full = write(&dir, "full.webp", gradient(960, 540, Shape::One));
        let half = write(&dir, "half.webp", {
            let mut small = image::DynamicImage::ImageRgb8(gradient(960, 540, Shape::One));
            small = small.resize_exact(480, 270, image::imageops::FilterType::Lanczos3);
            small.into_rgb8()
        });
        let other = write(&dir, "other.webp", gradient(960, 540, Shape::Another));

        let (full, half, other) = (
            phash(&full).unwrap(),
            phash(&half).unwrap(),
            phash(&other).unwrap(),
        );

        assert!(
            alike(&full, &half, MAX_DISTANCE),
            "the same picture scaled down differs by {} bits",
            distance(&full, &half)
        );
        assert!(
            !alike(&full, &other, MAX_DISTANCE),
            "a different picture only differs by {} bits",
            distance(&full, &other)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn undecodable_bytes_are_reported_rather_than_hashed() {
        let dir = dir("apod-phash-broken");
        let path = dir.join("broken.webp");
        std::fs::write(&path, b"not an image").unwrap();

        assert!(phash(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
