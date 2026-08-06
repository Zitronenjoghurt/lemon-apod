use anyhow::{Context, Result};
use std::time::Duration;

pub enum Response {
    Body(Vec<u8>),
    NotFound,
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
        let mut attempt = 0;

        loop {
            attempt += 1;

            match self.try_get(url).await {
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

    async fn try_get(&self, url: &str) -> Attempt {
        let response = match self.http.get(url).send().await {
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

        match response.bytes().await {
            Ok(bytes) => Attempt::Done(Response::Body(bytes.to_vec())),
            Err(error) => Attempt::Retryable(
                anyhow::Error::new(error).context(format!("reading the body of {url}")),
            ),
        }
    }
}
