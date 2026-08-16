use crate::meta::{self, Target};
use crate::state::ServerState;
use axum::extract::State;
use axum::http::{HeaderValue, Uri, header};
use axum::response::{Html, IntoResponse, Response};

pub async fn spa(State(state): State<ServerState>, uri: Uri) -> Response {
    let path = uri.path();

    let page = match meta::target(path) {
        Target::Entry(date) => match looked_up(state.store.entry(date).await, path) {
            Some(entry) => state.shell.entry_page(&entry),
            None => state.shell.page(path),
        },
        Target::Picture(date) => {
            match looked_up(state.store.picture_appearances(date).await, path) {
                Some(found) => state.shell.picture_page(path, &found),
                None => state.shell.page(path),
            }
        }
        Target::Resource(id) => match looked_up(state.store.resource(id, 0, 1).await, path) {
            Some(found) => state.shell.resource_page(path, &found.resource),
            None => state.shell.page(path),
        },
        Target::Fixed => state.shell.page(path),
    };

    let mut response = Html(page).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

/// A page that cannot be looked up still has to render, only with plainer tags
fn looked_up<T, E: std::fmt::Display>(found: Result<Option<T>, E>, path: &str) -> Option<T> {
    found.unwrap_or_else(|error| {
        tracing::error!("rendering {path}: {error}");
        None
    })
}
