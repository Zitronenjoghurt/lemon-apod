use anyhow::{Context, Result};
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;
use re_mp4::{Mp4, TrackKind};

const CANDIDATES: usize = 12;
const SCORE_STEP: usize = 4;

const START_CODE: [u8; 4] = [0, 0, 0, 1];

pub struct Frame {
    pub rgb: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub fn poster_frame(bytes: &[u8]) -> Result<Frame> {
    let mp4 = Mp4::read_bytes(bytes).context("reading the mp4 container")?;

    let track = mp4
        .tracks()
        .values()
        .find(|track| track.kind == Some(TrackKind::Video))
        .context("the mp4 has no video track")?;

    let codec = track.codec_string(&mp4).unwrap_or_default();
    anyhow::ensure!(
        codec.starts_with("avc1"),
        "only H.264 is supported, this track is '{codec}'"
    );

    let config = track
        .raw_codec_config(&mp4)
        .context("the video track carries no avcC configuration")?;
    let avcc = Avcc::parse(&config)?;

    let keyframes: Vec<_> = track
        .samples
        .iter()
        .filter(|sample| sample.is_sync)
        .collect();
    anyhow::ensure!(!keyframes.is_empty(), "the video track has no keyframe");

    let mut decoder = Decoder::new().context("starting the H.264 decoder")?;
    let mut packet = Vec::new();
    let mut best: Option<(f64, Frame)> = None;

    for index in spread(keyframes.len(), CANDIDATES) {
        let data = bytes
            .get(keyframes[index].byte_range())
            .context("a sample points outside the file")?;

        packet.clear();
        packet.extend_from_slice(&avcc.parameter_sets);
        to_annex_b(data, avcc.length_size, &mut packet);

        if let Ok(Some(yuv)) = decoder.decode(&packet) {
            let score = detail(&yuv);
            if best.as_ref().is_none_or(|(previous, _)| score > *previous) {
                best = Some((score, to_rgb(&yuv)));
            }
        }
    }

    best.map(|(_, frame)| frame)
        .context("the decoder produced no frame")
}

fn spread(len: usize, count: usize) -> impl Iterator<Item = usize> {
    let step = len.div_ceil(count).max(1);
    (0..len).step_by(step)
}

fn detail(yuv: &openh264::decoder::DecodedYUV<'_>) -> f64 {
    let (width, height) = yuv.dimensions();
    let stride = yuv.strides().0;
    let luma = yuv.y();

    let samples: Vec<u8> = (0..height)
        .step_by(SCORE_STEP)
        .flat_map(|row| {
            (0..width)
                .step_by(SCORE_STEP)
                .filter_map(move |column| luma.get(row * stride + column).copied())
        })
        .collect();

    if samples.is_empty() {
        return 0.0;
    }

    let mean = samples.iter().map(|value| *value as f64).sum::<f64>() / samples.len() as f64;
    samples
        .iter()
        .map(|value| (*value as f64 - mean).abs())
        .sum::<f64>()
        / samples.len() as f64
}

fn to_rgb(yuv: &openh264::decoder::DecodedYUV<'_>) -> Frame {
    let (width, height) = yuv.dimensions();
    let mut rgb = vec![0u8; width * height * 3];
    yuv.write_rgb8(&mut rgb);

    Frame {
        rgb,
        width: width as u32,
        height: height as u32,
    }
}

struct Avcc {
    length_size: usize,
    parameter_sets: Vec<u8>,
}

impl Avcc {
    fn parse(raw: &[u8]) -> Result<Self> {
        anyhow::ensure!(raw.len() > 5, "the avcC box is truncated");

        let length_size = (raw[4] & 0b11) as usize + 1;
        let mut parameter_sets = Vec::new();
        let mut at = 5;

        let sequence_sets = raw[at] & 0b0001_1111;
        at += 1;
        read_sets(raw, &mut at, sequence_sets, &mut parameter_sets)?;

        let picture_sets = *raw.get(at).context("the avcC box is truncated")?;
        at += 1;
        read_sets(raw, &mut at, picture_sets, &mut parameter_sets)?;

        anyhow::ensure!(
            !parameter_sets.is_empty(),
            "the avcC box carries no parameter sets"
        );
        Ok(Self {
            length_size,
            parameter_sets,
        })
    }
}

fn read_sets(raw: &[u8], at: &mut usize, count: u8, out: &mut Vec<u8>) -> Result<()> {
    for _ in 0..count {
        let header: [u8; 2] = raw
            .get(*at..*at + 2)
            .context("the avcC box is truncated")?
            .try_into()
            .expect("a two byte slice is a two byte array");
        *at += 2;

        let length = u16::from_be_bytes(header) as usize;
        let set = raw
            .get(*at..*at + length)
            .context("a parameter set runs past the end of the avcC box")?;
        *at += length;

        out.extend_from_slice(&START_CODE);
        out.extend_from_slice(set);
    }
    Ok(())
}

fn to_annex_b(sample: &[u8], length_size: usize, out: &mut Vec<u8>) {
    let mut at = 0;

    while at + length_size <= sample.len() {
        let length = sample[at..at + length_size]
            .iter()
            .fold(0usize, |acc, byte| (acc << 8) | *byte as usize);
        at += length_size;

        let Some(nal) = sample.get(at..at + length) else {
            return;
        };
        out.extend_from_slice(&START_CODE);
        out.extend_from_slice(nal);
        at += length;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_length_prefixed_units_as_a_start_code_stream() {
        let sample = [0, 0, 0, 2, 0x65, 0xAA, 0, 0, 0, 1, 0x41];
        let mut out = Vec::new();
        to_annex_b(&sample, 4, &mut out);

        assert_eq!(out, [0, 0, 0, 1, 0x65, 0xAA, 0, 0, 0, 1, 0x41]);
    }

    #[test]
    fn a_truncated_unit_ends_the_rewrite_rather_than_panicking() {
        let mut out = Vec::new();
        to_annex_b(&[0, 0, 0, 8, 0x65], 4, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn reads_the_parameter_sets_out_of_an_avcc_box() {
        let raw = [
            1, 0x64, 0, 0x28, // version, profile, compatibility, level
            0xFF, // reserved | lengthSizeMinusOne = 3
            0xE1, // reserved | one SPS
            0, 3, 0x67, 0x64, 0x28, // SPS
            1,    // one PPS
            0, 2, 0x68, 0xEE, // PPS
        ];

        let avcc = Avcc::parse(&raw).unwrap();
        assert_eq!(avcc.length_size, 4);
        assert_eq!(
            avcc.parameter_sets,
            [0, 0, 0, 1, 0x67, 0x64, 0x28, 0, 0, 0, 1, 0x68, 0xEE]
        );
    }

    #[test]
    fn rejects_a_truncated_avcc_box() {
        assert!(Avcc::parse(&[1, 0x64, 0, 0x28, 0xFF]).is_err());
    }

    #[test]
    fn rejects_bytes_that_are_not_an_mp4() {
        assert!(poster_frame(b"not an mp4 at all").is_err());
    }

    #[test]
    fn spreads_candidates_over_the_keyframes_it_has() {
        assert_eq!(spread(5, 12).collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
        assert_eq!(spread(1, 12).collect::<Vec<_>>(), [0]);
        assert_eq!(spread(0, 12).count(), 0);
        assert!(spread(600, 12).count() <= 12);
        assert!(spread(600, 12).last().unwrap() > 500, "reaches the end");
    }
}
