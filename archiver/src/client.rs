use anyhow::{Context, Result};
use std::time::Duration;

pub enum Response {
    Body(Vec<u8>),
    NotFound,
}

#[derive(Debug, Clone, Copy)]
pub struct Limit {
    pub max_bytes: u64,
    pub timeout: Duration,
}

enum Attempt {
    Done(Response),
    Retryable(anyhow::Error),
    Fatal(anyhow::Error),
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    max_retries: u32,
}

impl Client {
    pub fn new(user_agent: &str, timeout: Duration, max_retries: u32) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(timeout)
            .build()
            .context("building the HTTP client")?;

        Ok(Self { http, max_retries })
    }

    pub async fn get(&self, url: &str) -> Result<Response> {
        self.fetch(url, None).await
    }

    pub async fn get_limited(&self, url: &str, limit: Limit) -> Result<Response> {
        self.fetch(url, Some(limit)).await
    }

    async fn fetch(&self, url: &str, limit: Option<Limit>) -> Result<Response> {
        let mut attempt = 0;

        loop {
            attempt += 1;

            match self.try_get(url, limit).await {
                Attempt::Done(response) => return Ok(response),
                Attempt::Fatal(error) => return Err(error),
                Attempt::Retryable(error) if attempt > self.max_retries => return Err(error),
                Attempt::Retryable(error) => {
                    let backoff = Duration::from_secs(2u64.pow(attempt.min(6)));
                    tracing::debug!(%url, attempt, ?backoff, "retrying after {error:#}");
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    async fn try_get(&self, url: &str, limit: Option<Limit>) -> Attempt {
        let mut request = self.http.get(url);
        if let Some(limit) = limit {
            request = request.timeout(limit.timeout);
        }

        let mut response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                return Attempt::Retryable(
                    anyhow::Error::new(error).context(format!("requesting {url}")),
                );
            }
        };

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Attempt::Done(Response::NotFound);
        }
        if status.is_server_error() {
            return Attempt::Retryable(anyhow::anyhow!("{url} returned {status}"));
        }
        if !status.is_success() {
            return Attempt::Fatal(anyhow::anyhow!("{url} returned {status}"));
        }

        let Some(limit) = limit else {
            return match response.bytes().await {
                Ok(bytes) => Attempt::Done(Response::Body(bytes.to_vec())),
                Err(error) => Attempt::Retryable(
                    anyhow::Error::new(error).context(format!("reading the body of {url}")),
                ),
            };
        };

        if let Some(declared) = response.content_length()
            && declared > limit.max_bytes
        {
            return Attempt::Fatal(too_large(url, declared, limit.max_bytes));
        }

        let mut body = Vec::new();
        loop {
            match response.chunk().await {
                Ok(None) => return Attempt::Done(Response::Body(body)),
                Ok(Some(chunk)) => {
                    if body.len() as u64 + chunk.len() as u64 > limit.max_bytes {
                        return Attempt::Fatal(too_large(
                            url,
                            limit.max_bytes + 1,
                            limit.max_bytes,
                        ));
                    }
                    body.extend_from_slice(&chunk);
                }
                Err(error) => {
                    return Attempt::Retryable(
                        anyhow::Error::new(error).context(format!("reading the body of {url}")),
                    );
                }
            }
        }
    }
}

fn too_large(url: &str, size: u64, max: u64) -> anyhow::Error {
    anyhow::anyhow!(
        "{url} is {:.0}MB, over the {:.0}MB limit",
        size as f64 / 1_048_576.0,
        max as f64 / 1_048_576.0
    )
}
