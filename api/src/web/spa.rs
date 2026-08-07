use crate::meta;
use crate::state::ServerState;
use axum::extract::State;
use axum::http::{HeaderValue, Uri, header};
use axum::response::{Html, IntoResponse, Response};

pub async fn spa(State(state): State<ServerState>, uri: Uri) -> Response {
    let page = match meta::entry_path(uri.path()) {
        Some(date) => match state.store.entry(date).await {
            Ok(Some(entry)) => state.shell.entry_page(&entry),
            Ok(None) => state.shell.default_page(),
            Err(error) => {
                tracing::error!("rendering {}: {error:#}", uri.path());
                state.shell.default_page()
            }
        },
        None => state.shell.default_page(),
    };

    let mut response = Html(page).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}
