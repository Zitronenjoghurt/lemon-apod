use crate::api::error::ApiResult;
use crate::state::ServerState;
use apod_core::ApodDate;
use axum::extract::State;
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use std::collections::BTreeSet;

pub async fn get_robots(State(state): State<ServerState>) -> Response {
    let body = format!(
        "User-agent: *\n\
         Allow: /\n\
         Disallow: /random\n\
         Disallow: /rating/vote\n\
         Disallow: /api/\n\
         Sitemap: {}/sitemap.xml\n",
        state.config.public_url
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

pub async fn get_sitemap(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let xml = state.sitemap.get_or_build(|| build(&state)).await?;
    Ok(crate::api::response::revalidated(
        &headers,
        &xml,
        "application/xml; charset=utf-8",
    ))
}

async fn build(state: &ServerState) -> ApiResult<String> {
    let dates = state.store.all_dates().await?;
    let base = &state.config.public_url;

    let mut xml = String::with_capacity(dates.len() * 96);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

    push_url(&mut xml, &format!("{base}/"), Some("daily"));
    for path in crate::meta::indexable_paths() {
        push_url(&mut xml, &format!("{base}{path}"), Some("weekly"));
    }

    let years: BTreeSet<String> = dates.iter().map(|date| date.format("%Y")).collect();
    for year in years.iter().rev() {
        push_url(&mut xml, &format!("{base}/archive/{year}"), Some("weekly"));
    }

    for date in ApodDate::KNOWN_MISSING {
        push_url(&mut xml, &format!("{base}/{date}"), None);
    }

    for date in &dates {
        push_url(&mut xml, &format!("{base}/{date}"), None);
    }

    xml.push_str("</urlset>\n");
    Ok(xml)
}

fn push_url(xml: &mut String, location: &str, change_frequency: Option<&str>) {
    xml.push_str("  <url>\n    <loc>");
    xml.push_str(&super::escape(location));
    xml.push_str("</loc>\n");
    if let Some(frequency) = change_frequency {
        xml.push_str("    <changefreq>");
        xml.push_str(frequency);
        xml.push_str("</changefreq>\n");
    }
    xml.push_str("  </url>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_reserved_xml_characters() {
        let mut xml = String::new();
        push_url(&mut xml, "https://x/a&b", None);
        assert!(xml.contains("https://x/a&amp;b"), "{xml}");
    }

    #[test]
    fn emits_a_change_frequency_only_where_given() {
        let mut xml = String::new();
        push_url(&mut xml, "https://x/", Some("daily"));
        push_url(&mut xml, "https://x/2024-03-05", None);

        assert!(xml.contains("<changefreq>daily</changefreq>"));
        assert!(!xml.contains("<loc>https://x/2024-03-05</loc>\n    <changefreq>"));
    }

    #[test]
    fn no_two_elements_share_a_line() {
        let mut xml = String::new();
        push_url(&mut xml, "https://x/", Some("daily"));
        push_url(&mut xml, "https://x/archive/2026", Some("weekly"));

        for line in xml.lines() {
            let opening_tags = line.matches('<').count() - line.matches("</").count();
            assert!(opening_tags <= 1, "two elements on one line: {line}");
        }
        assert!(!xml.contains("</loc><changefreq>"));
    }
}
