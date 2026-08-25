use crate::date::ApodDate;
use std::collections::HashMap;

/// A 256-bit difference hash: sixteen rows of sixteen comparisons between horizontally adjacent
/// pixels of a 17x16 grayscale copy of the thumbnail.
pub const PHASH_BYTES: usize = 32;

/// How many of those 256 bits may differ and still be the same picture.
pub const MAX_DISTANCE: u32 = 8;

/// How many of the 256 bits have to fall on the less common side for a hash to mean anything.
///
/// A difference hash records shape and throws brightness away, so a picture with no shape in it
/// hashes to all zeros, and two such hashes match each other perfectly while saying nothing. This
/// is the floor for calling a hash a picture at all; below it an entry is matched by its media URL
/// or not at all.
pub const MIN_DETAIL: u32 = 4;

/// What identifies one entry's picture: the thumbnail's hash, and the media URL the page pointed
/// at. Either may be missing, and an entry with neither cannot be grouped at all.
#[derive(Debug, Clone)]
pub struct Fingerprint {
    pub date: ApodDate,
    pub media_url: Option<String>,
    pub phash: Option<Vec<u8>>,
}

/// One picture and every date it ran on, earliest first. A picture that ran once is not a group,
/// so this always holds at least two dates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PictureGroup {
    pub dates: Vec<ApodDate>,
}

impl PictureGroup {
    pub fn id(&self) -> ApodDate {
        self.dates[0]
    }
}

pub fn alike(a: &[u8], b: &[u8], max: u32) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut differing = 0;
    for (left, right) in a.as_chunks::<8>().0.iter().zip(b.as_chunks::<8>().0) {
        differing += (u64::from_be_bytes(*left) ^ u64::from_be_bytes(*right)).count_ones();
        if differing > max {
            return false;
        }
    }

    differing <= max
}

pub fn has_detail(phash: &[u8]) -> bool {
    let set: u32 = phash.iter().map(|byte| byte.count_ones()).sum();
    let bits = (phash.len() * 8) as u32;
    set >= MIN_DETAIL && set <= bits.saturating_sub(MIN_DETAIL)
}

pub fn distance(a: &[u8], b: &[u8]) -> u32 {
    if a.len() != b.len() {
        return u32::MAX;
    }

    a.iter()
        .zip(b)
        .map(|(left, right)| (left ^ right).count_ones())
        .sum()
}

/// Gather fingerprints into the pictures they belong to, keeping only the pictures that ran more
/// than once.
pub fn group(prints: &[Fingerprint]) -> Vec<PictureGroup> {
    let mut union = Union::new(prints.len());

    let mut by_url: HashMap<&str, usize> = HashMap::new();
    for (index, print) in prints.iter().enumerate() {
        let Some(url) = print.media_url.as_deref().filter(|url| !url.is_empty()) else {
            continue;
        };
        match by_url.get(url) {
            Some(&first) => union.join(first, index),
            None => {
                by_url.insert(url, index);
            }
        }
    }

    let hashed: Vec<(usize, &[u8])> = prints
        .iter()
        .enumerate()
        .filter_map(|(index, print)| {
            let phash = print.phash.as_deref()?;
            (phash.len() == PHASH_BYTES && has_detail(phash)).then_some((index, phash))
        })
        .collect();

    for (position, &(index, phash)) in hashed.iter().enumerate() {
        for &(other, other_phash) in &hashed[position + 1..] {
            if alike(phash, other_phash, MAX_DISTANCE) {
                union.join(index, other);
            }
        }
    }

    let mut gathered: HashMap<usize, Vec<ApodDate>> = HashMap::new();
    for (index, print) in prints.iter().enumerate() {
        gathered
            .entry(union.root(index))
            .or_default()
            .push(print.date);
    }

    let mut groups: Vec<PictureGroup> = gathered
        .into_values()
        .filter(|dates| dates.len() > 1)
        .map(|mut dates| {
            dates.sort_unstable();
            PictureGroup { dates }
        })
        .collect();

    groups.sort_unstable_by_key(PictureGroup::id);
    groups
}

struct Union {
    parent: Vec<usize>,
}

impl Union {
    fn new(len: usize) -> Self {
        Self {
            parent: (0..len).collect(),
        }
    }

    fn root(&mut self, mut index: usize) -> usize {
        while self.parent[index] != index {
            self.parent[index] = self.parent[self.parent[index]];
            index = self.parent[index];
        }
        index
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.root(a), self.root(b));
        if a != b {
            self.parent[b.max(a)] = b.min(a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: u8) -> Vec<u8> {
        vec![byte; PHASH_BYTES]
    }

    fn print(date: &str, url: Option<&str>, phash: Option<Vec<u8>>) -> Fingerprint {
        Fingerprint {
            date: date.parse().unwrap(),
            media_url: url.map(str::to_owned),
            phash,
        }
    }

    #[test]
    fn a_flipped_bit_is_still_the_same_picture_but_a_flipped_byte_is_not() {
        let mut near = hash(0);
        near[3] = 0b0000_0001;
        let mut far = hash(0);
        far[3] = 0b1111_1111;

        assert_eq!(distance(&hash(0), &near), 1);
        assert!(alike(&hash(0), &near, MAX_DISTANCE));
        assert_eq!(distance(&hash(0), &far), 8);
        assert!(alike(&hash(0), &far, MAX_DISTANCE));

        far[4] = 0b0000_0001;
        assert!(!alike(&hash(0), &far, MAX_DISTANCE));
    }

    #[test]
    fn hashes_of_different_lengths_are_never_alike() {
        assert!(!alike(&hash(0), &[0, 0], MAX_DISTANCE));
        assert_eq!(distance(&hash(0), &[0, 0]), u32::MAX);
    }

    #[test]
    fn a_hash_with_no_shape_in_it_is_not_worth_matching_on() {
        assert!(!has_detail(&hash(0)), "a flat colour");
        assert!(!has_detail(&hash(0xff)), "and its inverse");
        assert!(has_detail(&hash(0b0101_0101)));

        let mut sparse = hash(0);
        sparse[0] = 0b0000_0011;
        assert!(!has_detail(&sparse), "two bits of 256 is not a picture");
        sparse[0] = 0b0000_1111;
        assert!(
            has_detail(&sparse),
            "a night shot has this little and no less"
        );
    }

    #[test]
    fn two_featureless_thumbnails_are_not_the_same_picture() {
        let groups = group(&[
            print("2020-12-27", Some("latte.jpg"), Some(hash(0))),
            print("2020-04-22", Some("twilight.jpg"), Some(hash(0))),
            print("2020-12-30", Some("conjunction.jpg"), Some(hash(0))),
        ]);

        assert!(
            groups.is_empty(),
            "nothing at all in a hash matches nothing at all in another"
        );
    }

    #[test]
    fn a_picture_that_ran_once_is_not_a_group() {
        let groups = group(&[
            print("2020-01-01", Some("a.jpg"), Some(hash(1))),
            print("2020-01-02", Some("b.jpg"), Some(hash(2))),
        ]);
        assert!(groups.is_empty());
    }

    #[test]
    fn a_still_moved_to_a_new_folder_groups_on_its_hash() {
        let groups = group(&[
            print("2019-09-20", Some("image/1909/saturn.jpg"), Some(hash(7))),
            print("2021-09-11", Some("image/2109/saturn.jpg"), Some(hash(7))),
        ]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].dates.len(), 2);
        assert_eq!(groups[0].id().to_string(), "2019-09-20");
    }

    #[test]
    fn a_rerun_video_groups_on_its_url_however_its_poster_frame_came_out() {
        let groups = group(&[
            print("2020-07-19", Some("lro.mp4"), Some(hash(0b0101_0101))),
            print("2025-10-04", Some("lro.mp4"), Some(hash(0b1010_1010))),
        ]);

        assert_eq!(groups.len(), 1, "the URL is the same file either way");
        assert_eq!(groups[0].dates.len(), 2);
    }

    #[test]
    fn one_picture_reached_by_either_edge_is_one_group() {
        let groups = group(&[
            print("2020-01-01", Some("same.mp4"), Some(hash(1))),
            print("2020-06-01", Some("same.mp4"), Some(hash(2))),
            print("2021-01-01", Some("moved.mp4"), Some(hash(2))),
        ]);

        assert_eq!(
            groups.len(),
            1,
            "url joins the first two, hash the last two"
        );
        assert_eq!(groups[0].dates.len(), 3);
        assert_eq!(groups[0].id().to_string(), "2020-01-01");
    }

    #[test]
    fn an_entry_with_nothing_to_compare_stands_alone() {
        let groups = group(&[
            print("2020-01-01", None, None),
            print("2020-01-02", None, None),
            print("2020-01-03", Some(""), None),
        ]);
        assert!(groups.is_empty(), "no url and no hash is not a match");
    }

    #[test]
    fn groups_come_back_in_date_order_however_the_rows_arrived() {
        let groups = group(&[
            print("2024-05-05", Some("b.jpg"), Some(hash(2))),
            print("2020-02-02", Some("a.jpg"), Some(hash(1))),
            print("2022-03-03", Some("b.jpg"), Some(hash(2))),
            print("2021-01-01", Some("a.jpg"), Some(hash(1))),
        ]);

        let ids: Vec<String> = groups.iter().map(|g| g.id().to_string()).collect();
        assert_eq!(ids, vec!["2020-02-02", "2022-03-03"]);
        assert_eq!(groups[0].dates[1].to_string(), "2021-01-01");
    }
}
