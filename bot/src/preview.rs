use crate::card;
use crate::config::Config;
use crate::store::Explanation;
use anyhow::{Context, Result, bail};
use apod_core::db::DbConfig;
use apod_core::{ApodDate, ApodReader};

pub const FLAG: &str = "--preview";

pub fn requested() -> bool {
    std::env::args().any(|arg| arg == FLAG)
}

pub async fn run(config: Config) -> Result<()> {
    let asked = std::env::args().skip_while(|arg| arg != FLAG).nth(1);

    let apod = ApodReader::open(DbConfig::read_only(&config.index_db))
        .await
        .with_context(|| format!("opening {}", config.index_db.display()))?;

    let entry = match asked.as_deref() {
        Some(raw) => {
            let date: ApodDate = raw
                .parse()
                .with_context(|| format!("'{raw}' is not a YYYY-MM-DD date"))?;
            apod.entry(date).await?
        }
        None => apod.latest().await?,
    };

    let Some(entry) = entry else {
        apod.db().close().await;
        bail!("the archive has no such entry");
    };

    let attachment = card::thumbnail(&config, &entry).await;

    println!("{} — {}", entry.date, entry.title);
    match &attachment {
        Some(attachment) => println!(
            "picture: {} ({} bytes, uploaded with the post)",
            attachment.filename,
            attachment.data.len()
        ),
        None => println!("picture: none on disk, the post would carry no image"),
    }

    for explanation in [Explanation::Full, Explanation::Teaser, Explanation::None] {
        let embed = card::embed(&config, &entry, explanation, attachment.as_ref());
        let json = serde_json::to_value(&embed)?;
        println!("\n=== {explanation} ({} characters) ===", weight(&json));
        println!("{}", serde_json::to_string_pretty(&json)?);
    }

    apod.db().close().await;
    Ok(())
}

fn weight(embed: &serde_json::Value) -> usize {
    let text = |value: Option<&serde_json::Value>| {
        value
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .chars()
            .count()
    };

    text(embed.get("title"))
        + text(embed.get("description"))
        + text(embed.get("footer").and_then(|footer| footer.get("text")))
        + text(embed.get("author").and_then(|author| author.get("name")))
        + embed
            .get("fields")
            .and_then(|fields| fields.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|row| text(row.get("name")) + text(row.get("value")))
                    .sum::<usize>()
            })
            .unwrap_or(0)
}
