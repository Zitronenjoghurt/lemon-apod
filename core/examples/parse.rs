//! Parse archived pages from disk and print what came out.
//!
//! This is the inner loop of parser refinement: point it at raw HTML, read the result, adjust the
//! extractors, run it again. Nothing here touches the network.
//!
//! ```text
//! cargo run -p apod-core --example parse -- 2024-03-05=/path/to/ap240305.html
//! ```

use apod_core::{ApodDate, parse, quality};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: parse <YYYY-MM-DD>=<file.html> [...]");
        std::process::exit(2);
    }

    for arg in args {
        let Some((date, path)) = arg.split_once('=') else {
            eprintln!("expected <YYYY-MM-DD>=<file.html>, got '{arg}'");
            continue;
        };

        let date: ApodDate = date.parse().expect("valid date");
        let bytes = std::fs::read(path).expect("readable file");

        println!("\n=== {date} ({path}) ===");
        match parse::parse_bytes(date, &bytes) {
            Err(error) => println!("  PARSE FAILED: {error}"),
            Ok(entry) => {
                println!("  title      {}", entry.title);
                println!("  title_raw  {:?}", entry.title_raw);
                println!("  credit     {:?}", entry.credit_text);
                println!("  copyright  {}", entry.has_copyright);
                println!("  media      {} {:?}", entry.media.kind, entry.media.url);
                println!("  media hd   {:?}", entry.media.hd_url);
                println!("  extra      {}", entry.extra_media.len());
                println!("  keywords   {:?}", entry.keywords);
                println!("  tomorrow   {:?}", entry.tomorrow_teaser);
                println!("  expl len   {}", entry.explanation_text.chars().count());
                println!("  expl text  {}", entry.summary_text(220));
                println!("  expl html  {}", truncate(&entry.explanation_html, 260));

                let issues = quality::quality_control(&entry);
                if issues.is_empty() {
                    println!("  quality    clean");
                } else {
                    for issue in issues {
                        println!("  quality    {issue}");
                    }
                }
            }
        }
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    text.chars().take(max).collect::<String>() + "…"
}
