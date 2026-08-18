use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::net::IpAddr;

const COHORT_BYTES: usize = 8;
const IPV4_PREFIX: u8 = 24;
const IPV6_PREFIX: u8 = 48;

pub fn cohort(secret: &[u8], address: IpAddr, user_agent: Option<&str>) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC takes a key of any length");
    mac.update(&prefix(address));
    mac.update(b"|");
    mac.update(class(user_agent).as_bytes());

    mac.finalize().into_bytes()[..COHORT_BYTES].to_vec()
}

fn prefix(address: IpAddr) -> Vec<u8> {
    match address {
        IpAddr::V4(address) => masked(&address.octets(), IPV4_PREFIX),
        IpAddr::V6(address) => masked(&address.octets(), IPV6_PREFIX),
    }
}

fn masked(octets: &[u8], bits: u8) -> Vec<u8> {
    octets
        .iter()
        .enumerate()
        .map(|(at, &octet)| {
            let taken = bits as usize;
            let start = at * 8;
            match taken.saturating_sub(start) {
                0 => 0,
                left if left >= 8 => octet,
                left => octet & !(0xffu8 >> left),
            }
        })
        .collect()
}

fn class(user_agent: Option<&str>) -> &'static str {
    let Some(agent) = user_agent else {
        return "none";
    };

    let agent = agent.to_ascii_lowercase();
    for (needle, name) in [
        ("firefox", "firefox"),
        ("edg/", "edge"),
        ("chrome", "chrome"),
        ("safari", "safari"),
        ("curl", "tool"),
        ("wget", "tool"),
        ("python", "tool"),
        ("bot", "bot"),
    ] {
        if agent.contains(needle) {
            return name;
        }
    }

    "other"
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"a deployment secret";

    fn v4(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    const FIREFOX: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:145.0) Gecko/20100101 Firefox/145.0";
    const CHROME: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/141.0 Safari/537.36";

    #[test]
    fn two_people_on_one_home_network_share_a_cohort() {
        assert_eq!(
            cohort(SECRET, v4("203.0.113.7"), Some(FIREFOX)),
            cohort(SECRET, v4("203.0.113.201"), Some(FIREFOX)),
            "a /24 is the neighbourhood, and that is the point"
        );
    }

    #[test]
    fn a_different_network_is_a_different_cohort() {
        assert_ne!(
            cohort(SECRET, v4("203.0.113.7"), Some(FIREFOX)),
            cohort(SECRET, v4("198.51.100.7"), Some(FIREFOX))
        );
    }

    #[test]
    fn a_script_does_not_hide_among_the_browsers_on_its_network() {
        let address = v4("203.0.113.7");
        assert_ne!(
            cohort(SECRET, address, Some(FIREFOX)),
            cohort(SECRET, address, Some("curl/8.7.1"))
        );
        assert_ne!(
            cohort(SECRET, address, Some(FIREFOX)),
            cohort(SECRET, address, None)
        );
    }

    #[test]
    fn a_browser_version_is_not_part_of_it() {
        let address = v4("203.0.113.7");
        let older = FIREFOX.replace("145.0", "128.0");

        assert_eq!(
            cohort(SECRET, address, Some(FIREFOX)),
            cohort(SECRET, address, Some(&older)),
            "a version is a fingerprint and this is not one"
        );
    }

    #[test]
    fn edge_is_not_chrome_even_though_it_says_it_is() {
        let address = v4("203.0.113.7");
        let edge = format!("{CHROME} Edg/141.0");

        assert_ne!(
            cohort(SECRET, address, Some(CHROME)),
            cohort(SECRET, address, Some(&edge))
        );
    }

    #[test]
    fn rotating_the_secret_breaks_every_link_that_came_before_it() {
        let address = v4("203.0.113.7");
        assert_ne!(
            cohort(SECRET, address, Some(FIREFOX)),
            cohort(b"the next secret", address, Some(FIREFOX))
        );
    }

    #[test]
    fn a_cohort_is_short_and_says_nothing_about_the_address_it_came_from() {
        let hashed = cohort(SECRET, v4("203.0.113.7"), Some(FIREFOX));
        assert_eq!(hashed.len(), COHORT_BYTES);
    }

    #[test]
    fn an_ipv6_address_is_cut_to_its_site_rather_than_its_interface() {
        let one: IpAddr = "2001:db8:abcd:1234::1".parse().unwrap();
        let two: IpAddr = "2001:db8:abcd:9999::abcd".parse().unwrap();
        let elsewhere: IpAddr = "2001:db8:ffff:1234::1".parse().unwrap();

        assert_eq!(
            cohort(SECRET, one, Some(FIREFOX)),
            cohort(SECRET, two, Some(FIREFOX)),
            "a rotating interface identifier must not mint a new cohort"
        );
        assert_ne!(
            cohort(SECRET, one, Some(FIREFOX)),
            cohort(SECRET, elsewhere, Some(FIREFOX))
        );
    }

    #[test]
    fn the_mask_keeps_the_network_and_drops_the_host() {
        assert_eq!(masked(&[203, 0, 113, 201], 24), vec![203, 0, 113, 0]);
        assert_eq!(masked(&[203, 0, 113, 201], 20), vec![203, 0, 112, 0]);
        assert_eq!(masked(&[203, 0, 113, 201], 32), vec![203, 0, 113, 201]);
        assert_eq!(masked(&[203, 0, 113, 201], 0), vec![0, 0, 0, 0]);
    }
}
