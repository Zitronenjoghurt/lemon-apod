use crate::config::Config;
use anyhow::{Context, Result, bail};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST: &[u8] = b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";

pub const FLAG: &str = "--health";

pub fn requested() -> bool {
    std::env::args().any(|arg| arg == FLAG)
}

pub async fn run() -> Result<()> {
    let config = Config::from_env().context("reading the port to probe")?;

    match tokio::time::timeout(TIMEOUT, probe(config.port)).await {
        Err(_) => bail!("/health did not answer within {TIMEOUT:?}"),
        Ok(result) => result,
    }
}

async fn probe(port: u16) -> Result<()> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .with_context(|| format!("connecting to 127.0.0.1:{port}"))?;

    stream
        .write_all(REQUEST)
        .await
        .context("sending the probe")?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .context("reading the reply")?;

    let status = String::from_utf8_lossy(&response);
    let status = status.lines().next().unwrap_or_default();

    if status.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        bail!("/health answered '{status}'")
    }
}
