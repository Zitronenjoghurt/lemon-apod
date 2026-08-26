use anyhow::{Context, Result};
use std::time::Duration;

pub enum Response {
    Body(Vec<u8>),
    NotFound,
    Redirected {
        status: u16,
        location: Option<String>,
    },
    Refused {
        status: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redirects {
    Follow,
    Refuse,
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
pub struct Clients {
    pub source: Client,
    pub media: Client,
}

impl Clients {
    pub fn new(user_agent: &str, timeout: Duration, max_retries: u32) -> Result<Self> {
        Ok(Self {
            source: Client::new(user_agent, timeout, max_retries, Redirects::Refuse)?,
            media: Client::new(user_agent, timeout, max_retries, Redirects::Follow)?,
        })
    }
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    max_retries: u32,
}

impl Client {
    pub fn new(
        user_agent: &str,
        timeout: Duration,
        max_retries: u32,
        redirects: Redirects,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(timeout)
            .redirect(match redirects {
                Redirects::Follow => reqwest::redirect::Policy::limited(10),
                Redirects::Refuse => reqwest::redirect::Policy::none(),
            })
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

    pub async fn post(&self, url: &str, headers: &[(&str, String)], body: String) -> Result<()> {
        let mut attempt = 0;

        loop {
            attempt += 1;

            match self.try_post(url, headers, &body).await {
                Attempt::Done(_) => return Ok(()),
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

    async fn try_post(&self, url: &str, headers: &[(&str, String)], body: &str) -> Attempt {
        let mut request = self.http.post(url).body(body.to_owned());
        for (name, value) in headers {
            request = request.header(*name, value);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                return Attempt::Retryable(
                    anyhow::Error::new(error).context(format!("posting to {url}")),
                );
            }
        };

        let status = response.status();
        if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Attempt::Retryable(anyhow::anyhow!("{url} returned {status}"));
        }
        if !status.is_success() {
            return Attempt::Fatal(anyhow::anyhow!("{url} returned {status}"));
        }

        Attempt::Done(Response::Body(Vec::new()))
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
        if status.is_redirection() {
            return Attempt::Done(Response::Redirected {
                status: status.as_u16(),
                location: response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
            });
        }
        if status.is_server_error() {
            return Attempt::Retryable(anyhow::anyhow!("{url} returned {status}"));
        }
        if !status.is_success() {
            return Attempt::Done(Response::Refused {
                status: status.as_u16(),
            });
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const LIMIT: Limit = Limit {
        max_bytes: 64 * 1024,
        timeout: Duration::from_secs(10),
    };

    fn client(redirects: Redirects) -> Client {
        Client::new("apod-test", Duration::from_secs(10), 0, redirects).unwrap()
    }

    async fn rejected(url: &str) -> String {
        match client(Redirects::Follow).get_limited(url, LIMIT).await {
            Err(error) => format!("{error:#}"),
            Ok(_) => panic!("the cap let {url} through"),
        }
    }

    async fn serving(head: &'static str, body: Vec<u8>, endless: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let body = body.clone();
                tokio::spawn(async move {
                    let _ = socket.read(&mut [0u8; 2048]).await;
                    if socket.write_all(head.as_bytes()).await.is_err() {
                        return;
                    }
                    loop {
                        if socket.write_all(&body).await.is_err() || !endless {
                            return;
                        }
                    }
                });
            }
        });

        format!("http://{address}/big.jpg")
    }

    #[tokio::test]
    async fn refuses_a_body_that_declares_itself_too_large() {
        let url = serving(
            "HTTP/1.1 200 OK\r\nContent-Length: 104857600\r\n\r\n",
            b"x".repeat(4096),
            true,
        )
        .await;

        assert!(rejected(&url).await.contains("limit"));
    }

    #[tokio::test]
    async fn stops_reading_a_chunked_body_that_never_ends() {
        let chunk = format!("2000\r\n{}\r\n", "x".repeat(8192));
        let url = serving(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
            chunk.into_bytes(),
            true,
        )
        .await;

        assert!(rejected(&url).await.contains("limit"));
    }

    #[tokio::test]
    async fn a_body_inside_the_cap_still_arrives_whole() {
        let body = b"x".repeat(1024);
        let url = serving(
            "HTTP/1.1 200 OK\r\nContent-Length: 1024\r\n\r\n",
            body,
            false,
        )
        .await;

        let Response::Body(bytes) = client(Redirects::Follow)
            .get_limited(&url, LIMIT)
            .await
            .unwrap()
        else {
            panic!("expected a body");
        };
        assert_eq!(bytes.len(), 1024);
    }

    async fn redirecting(status: u16, location: Option<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let head = match location {
            Some(location) => format!(
                "HTTP/1.1 {status} Moved\r\nLocation: {location}\r\nContent-Length: 0\r\n\r\n"
            ),
            None => format!("HTTP/1.1 {status} Moved\r\nContent-Length: 0\r\n\r\n"),
        };

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let head = head.clone();
                tokio::spawn(async move {
                    let _ = socket.read(&mut [0u8; 2048]).await;
                    let _ = socket.write_all(head.as_bytes()).await;
                });
            }
        });

        format!("http://{address}/ap260825.html")
    }

    async fn counting() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                counter.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let _ = socket.read(&mut [0u8; 2048]).await;
                    let _ = socket
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nmoved")
                        .await;
                });
            }
        });

        (format!("http://{address}/moved"), hits)
    }

    #[tokio::test]
    async fn the_source_client_reports_a_redirect_without_following_it() {
        let (target, hits) = counting().await;
        let url = redirecting(301, Some(target.clone())).await;

        let Response::Redirected { status, location } =
            client(Redirects::Refuse).get(&url).await.unwrap()
        else {
            panic!("expected a redirect");
        };

        assert_eq!(status, 301);
        assert_eq!(location.as_deref(), Some(target.as_str()));
        assert_eq!(hits.load(Ordering::SeqCst), 0, "the target was requested");
    }

    #[tokio::test]
    async fn a_redirect_without_a_location_still_comes_back_as_one() {
        let url = redirecting(302, None).await;

        let Response::Redirected { status, location } =
            client(Redirects::Refuse).get(&url).await.unwrap()
        else {
            panic!("expected a redirect");
        };

        assert_eq!(status, 302);
        assert_eq!(location, None);
    }

    #[tokio::test]
    async fn third_party_media_still_follows_a_redirect() {
        let (target, hits) = counting().await;
        let url = redirecting(302, Some(target)).await;

        let Response::Body(bytes) = client(Redirects::Follow).get(&url).await.unwrap() else {
            panic!("expected the redirect to be followed");
        };

        assert_eq!(bytes, b"moved");
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
