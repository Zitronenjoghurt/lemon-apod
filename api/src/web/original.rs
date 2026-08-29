use crate::state::ServerState;
use apod_core::ApodDate;
use apod_core::original::{APOD_HOME, Original};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;

const CSP: &str = "default-src 'none'; \
     img-src 'self' https: data:; \
     media-src https:; \
     frame-src https:; \
     style-src 'unsafe-inline'; \
     script-src 'none'; \
     object-src 'none'; \
     base-uri 'none'; \
     form-action 'none'; \
     frame-ancestors 'none'";

const CACHE: &str = "public, max-age=3600";

async fn get_original(State(state): State<ServerState>, Path(date): Path<String>) -> Response {
    let Ok(date) = date.parse::<ApodDate>() else {
        return missing(None);
    };

    let entry = match state.store.entry(date).await {
        Ok(Some(entry)) => entry,
        Ok(None) => return missing(Some(date)),
        Err(error) => {
            tracing::error!(%date, "reading the entry behind its original view: {error}");
            return missing(Some(date));
        }
    };

    let archived = std::fs::read(state.config.html_dir.join(date.html_path()));
    let view = Original {
        entry: &entry,
        public_url: &state.config.public_url,
        fetched_at: match archived.is_ok() {
            true => state.archive.legacy_fetched_at(date).await,
            false => None,
        },
    };

    let page = match archived {
        Ok(bytes) => view.archived(&bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => view.reconstructed(),
        Err(error) => {
            tracing::error!(%date, "reading the archived page: {error}");
            return missing(Some(date));
        }
    };

    served(StatusCode::OK, page)
}

fn missing(date: Option<ApodDate>) -> Response {
    let (heading, body) = match date {
        Some(date) => (
            format!("No archived page for {date}"),
            format!(
                "<p>The archive holds no original page for this date. \
                 <a href=\"/{date}\">Look it up in the archive</a>, or see \
                 <a href=\"{APOD_HOME}\" rel=\"noopener\">APOD itself</a>.</p>"
            ),
        ),
        None => (
            "Not a date".to_owned(),
            format!(
                "<p>An original view is addressed by date, as <code>/YYYY-MM-DD/original</code>. \
                 <a href=\"/\">Go to the archive</a>, or see \
                 <a href=\"{APOD_HOME}\" rel=\"noopener\">APOD itself</a>.</p>"
            ),
        ),
    };

    served(
        StatusCode::NOT_FOUND,
        format!(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
             <meta name=\"robots\" content=\"noindex, follow\">\n\
             <title>{heading}</title>\n</head>\n\
             <body style=\"font-family:system-ui,sans-serif;max-width:40rem;margin:4rem auto;\
             padding:0 1rem\">\n<h1>{heading}</h1>\n{body}\n</body>\n</html>\n"
        ),
    )
}

fn served(status: StatusCode, page: String) -> Response {
    fn set(response: &mut Response, name: HeaderName, value: &'static str) {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }

    let mut response = (status, Html(page)).into_response();
    set(&mut response, header::CONTENT_SECURITY_POLICY, CSP);
    set(&mut response, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(&mut response, header::X_FRAME_OPTIONS, "DENY");
    set(
        &mut response,
        header::REFERRER_POLICY,
        "strict-origin-when-cross-origin",
    );
    set(
        &mut response,
        header::CACHE_CONTROL,
        match status {
            StatusCode::OK => CACHE,
            _ => "no-cache",
        },
    );
    response
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/{date}/original", get(get_original))
}
