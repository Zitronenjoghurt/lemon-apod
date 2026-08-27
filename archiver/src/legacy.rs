use crate::archive::{ArchiveStore, FetchRow, Source};
use crate::client::{Client, Redirects, Response};
use crate::config::Config;
use crate::fetch::{sha256, write_atomically};
use crate::progress;
use crate::reparse;
use anyhow::{Context, Result, bail, ensure};
use apod_core::ApodDate;
use chrono::Utc;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MANIFEST: &str = "manifest.sha256";
const FETCHES: &str = "fetches.jsonl";
const README: &str = "README.md";
const HTML_ROOT: &str = "html";
const SUMS: &str = "SHA256SUMS";

const DESCRIPTION: &str = include_str!("../templates/legacy-archive.md");

const COMPRESSION: i32 = 19;
const NAMED: usize = 20;

pub async fn export(cfg: &Config, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;

    let archive = ArchiveStore::open(&cfg.archive_db).await?;
    let rows = archive.fetch_rows(Source::Legacy).await?;
    let dates = reparse::archived_dates(&cfg.html_dir, "html")?;
    ensure!(
        !dates.is_empty(),
        "{} holds no pages, so there is nothing to export",
        cfg.html_dir.display()
    );

    let coverage = survey(cfg, &dates)?;
    let contents = gather(cfg, &dates, &rows, &coverage)?;
    let stem = format!("legacy-html-{}", Utc::now().format("%Y-%m-%d"));

    let tarball = write_archive(out, &format!("{stem}.tar.zst"), &contents, pack_tar_zst)?;
    let zipped = write_archive(out, &format!("{stem}.zip"), &contents, pack_zip)?;

    let manifest = out.join(MANIFEST);
    let rendered = entry(&contents, MANIFEST).expect("gather always renders a manifest");
    std::fs::write(&manifest, rendered)
        .with_context(|| format!("writing {}", manifest.display()))?;

    let sums = out.join(SUMS);
    std::fs::write(
        &sums,
        render_manifest(&BTreeMap::from([
            (tarball.name.clone(), tarball.digest.clone()),
            (zipped.name.clone(), zipped.digest.clone()),
            (MANIFEST.to_owned(), sha256(rendered)),
        ])),
    )
    .with_context(|| format!("writing {}", sums.display()))?;

    println!("{}", out.join(&tarball.name).display());
    println!("{}", out.join(&zipped.name).display());
    println!("{}", manifest.display());
    println!("{}", sums.display());
    println!();
    println!("coverage");
    println!("  first           {}", coverage.first);
    println!("  last            {}", coverage.last);
    println!("  files           {}", coverage.files);
    println!("  bytes           {}", megabytes(coverage.bytes));
    println!(
        "  gaps            {} dates in the range have no file",
        coverage.gaps
    );
    println!("  fetch state     {} rows", rows.len());
    println!();
    println!(
        "{:<32}{:>9}  {}",
        tarball.name,
        megabytes(tarball.bytes),
        tarball.digest
    );
    println!(
        "{:<32}{:>9}  {}",
        zipped.name,
        megabytes(zipped.bytes),
        zipped.digest
    );

    Ok(())
}

type Contents = [(String, Vec<u8>)];

fn gather(
    cfg: &Config,
    dates: &[ApodDate],
    rows: &[FetchRow],
    coverage: &Coverage,
) -> Result<Vec<(String, Vec<u8>)>> {
    let mut contents = vec![
        (README.to_owned(), coverage.render().into_bytes()),
        (FETCHES.to_owned(), render_fetches(rows)?.into_bytes()),
    ];

    let bar = progress::bar("reading", dates.len());
    for &date in dates {
        bar.set_message(date.to_string());
        let path = cfg.html_path(date);
        let page = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        contents.push((tar_path(date), page));
        bar.inc(1);
    }
    bar.finish_and_clear();

    let manifest: BTreeMap<String, String> = contents
        .iter()
        .map(|(path, body)| (path.clone(), sha256(body)))
        .collect();
    contents.push((MANIFEST.to_owned(), render_manifest(&manifest).into_bytes()));

    Ok(contents)
}

struct Written {
    name: String,
    bytes: u64,
    digest: String,
}

fn write_archive(
    out: &Path,
    name: &str,
    contents: &Contents,
    pack: fn(&Contents) -> Result<Vec<u8>>,
) -> Result<Written> {
    let spinner = progress::spinner("packing", name.to_owned());
    let packed = pack(contents)?;
    spinner.finish_and_clear();

    let path = out.join(name);
    std::fs::write(&path, &packed).with_context(|| format!("writing {}", path.display()))?;

    Ok(Written {
        name: name.to_owned(),
        bytes: packed.len() as u64,
        digest: sha256(&packed),
    })
}

fn pack_tar_zst(contents: &Contents) -> Result<Vec<u8>> {
    let mut builder = tar::Builder::new(
        zstd::Encoder::new(Vec::new(), COMPRESSION).context("starting the compressed stream")?,
    );

    for (path, body) in contents {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);

        builder
            .append_data(&mut header, path, body.as_slice())
            .with_context(|| format!("adding {path} to the tarball"))?;
    }

    builder
        .into_inner()
        .context("finishing the tarball")?
        .finish()
        .context("finishing the compressed stream")
}

fn pack_zip(contents: &Contents) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);

    for (path, body) in contents {
        writer
            .start_file(path, options)
            .with_context(|| format!("adding {path} to the zip"))?;
        writer
            .write_all(body)
            .with_context(|| format!("writing {path} into the zip"))?;
    }

    Ok(writer.finish().context("finishing the zip")?.into_inner())
}

fn entry<'a>(contents: &'a [(String, Vec<u8>)], path: &str) -> Option<&'a [u8]> {
    contents
        .iter()
        .find(|(name, _)| name == path)
        .map(|(_, body)| body.as_slice())
}

struct Coverage {
    first: ApodDate,
    last: ApodDate,
    files: usize,
    bytes: u64,
    gaps: usize,
}

fn survey(cfg: &Config, dates: &[ApodDate]) -> Result<Coverage> {
    let mut bytes = 0;
    for &date in dates {
        let path = cfg.html_path(date);
        bytes += std::fs::metadata(&path)
            .with_context(|| format!("measuring {}", path.display()))?
            .len();
    }

    let first = dates[0];
    let last = dates[dates.len() - 1];
    let held: BTreeSet<ApodDate> = dates.iter().copied().collect();

    Ok(Coverage {
        first,
        last,
        files: dates.len(),
        bytes,
        gaps: (first.days()..=last.days())
            .map(ApodDate::from_days)
            .filter(|date| !date.is_known_missing() && !held.contains(date))
            .count(),
    })
}

impl Coverage {
    fn render(&self) -> String {
        let gaps = match self.gaps {
            0 => format!(
                "none, beyond the {} dates APOD never published",
                ApodDate::KNOWN_MISSING.len()
            ),
            n => format!("{n} dates in the range have no file"),
        };

        DESCRIPTION
            .replace("{files}", &self.files.to_string())
            .replace("{first}", &self.first.to_string())
            .replace("{last}", &self.last.to_string())
            .replace("{gaps}", &gaps)
            .replace("{bytes}", &self.bytes.to_string())
    }
}

pub async fn import(
    cfg: &Config,
    source: Option<String>,
    expected: Option<String>,
    force: bool,
) -> Result<()> {
    let snapshot = obtain(cfg, source).await?;

    if let Some(expected) = expected {
        let expected = expected.trim().to_ascii_lowercase();
        let digest = sha256(&snapshot);
        ensure!(
            digest == expected,
            "the snapshot hashes to {digest}, not the {expected} that was asked for; nothing \
             has been read out of it"
        );
        println!("sha256     {digest}");
    }

    let contents = unpack(&snapshot)?;
    let plan = inspect(cfg, &contents, force)?;

    let written = restore(cfg, &contents, &plan)?;
    let archive = ArchiveStore::open(&cfg.archive_db).await?;
    let seeded = archive.seed(&plan.fetches).await?;

    println!("imported   {written}");
    println!("identical  {}", plan.identical);
    println!("conflicted {}", plan.conflicted.len());
    println!("seeded     {seeded} of {} fetch rows", plan.fetches.len());

    if plan.conflicted.is_empty() {
        return Ok(());
    }

    for date in plan.conflicted.iter().take(NAMED) {
        println!("  differs   {date}");
    }
    if plan.conflicted.len() > NAMED {
        println!("  ... and {} more", plan.conflicted.len() - NAMED);
    }

    let count = plan.conflicted.len();
    bail!(
        "the snapshot disagrees with what is already on disk for {count} date{}; nothing was \
         overwritten. Pass --force to take the snapshot's copy",
        if count == 1 { "" } else { "s" }
    )
}

async fn obtain(cfg: &Config, source: Option<String>) -> Result<Vec<u8>> {
    let source = match source.or_else(|| cfg.legacy_archive_url.clone()) {
        Some(source) => source,
        None => bail!(
            "there is no snapshot to import: pass a path or a URL, or set \
             APOD_LEGACY_ARCHIVE_URL to the published release asset"
        ),
    };

    if !source.starts_with("http://") && !source.starts_with("https://") {
        let path = PathBuf::from(&source);
        return std::fs::read(&path).with_context(|| format!("reading {}", path.display()));
    }

    let client = Client::new(
        &cfg.user_agent,
        cfg.fetch_timeout,
        cfg.fetch_max_retries,
        Redirects::Follow,
    )?;

    let spinner = progress::spinner("downloading", source.clone());
    let body = match client.get(&source).await? {
        Response::Body(body) => body,
        Response::NotFound => bail!("{source} is not there"),
        Response::Redirected { status, .. } | Response::Refused { status } => {
            bail!("{source} answered {status}")
        }
    };
    progress::done(
        &spinner,
        format!("downloaded {}", megabytes(body.len() as u64)),
    );

    Ok(body)
}

#[derive(Debug, Default)]
struct Plan {
    write: BTreeSet<String>,
    identical: usize,
    conflicted: Vec<ApodDate>,
    fetches: Vec<FetchRow>,
}

fn inspect(cfg: &Config, contents: &Contents, force: bool) -> Result<Plan> {
    let spinner = progress::spinner("verifying", "checking the snapshot against its manifest");
    let mut manifest: Option<BTreeMap<String, String>> = None;
    let mut hashes: BTreeMap<String, String> = BTreeMap::new();
    let mut plan = Plan::default();

    for (path, body) in contents {
        if path == MANIFEST {
            let text = std::str::from_utf8(body).context("manifest.sha256 is not UTF-8")?;
            manifest = Some(parse_manifest(text)?);
            continue;
        }

        let digest = sha256(body);
        hashes.insert(path.clone(), digest.clone());

        if path == FETCHES {
            plan.fetches = parse_fetches(body)?;
            continue;
        }

        if path == README {
            continue;
        }

        let date = entry_date(path)
            .with_context(|| format!("{path} is not a path a legacy snapshot can hold"))?;
        classify(cfg, date, path.clone(), &digest, force, &mut plan)?;
    }
    spinner.finish_and_clear();

    let manifest = manifest.context(
        "the snapshot carries no manifest.sha256, so none of it can be vouched for; nothing \
         has been written",
    )?;

    for (path, digest) in &hashes {
        let expected = manifest.get(path).with_context(|| {
            format!(
                "{} is in the snapshot but not in its manifest; nothing has been written",
                named(path)
            )
        })?;
        ensure!(
            expected == digest,
            "{} does not match the manifest, so the snapshot is damaged; nothing has been written",
            named(path)
        );
    }

    for path in manifest.keys() {
        ensure!(
            hashes.contains_key(path),
            "{} is in the manifest but missing from the snapshot; nothing has been written",
            named(path)
        );
    }

    Ok(plan)
}

fn classify(
    cfg: &Config,
    date: ApodDate,
    path: String,
    digest: &str,
    force: bool,
    plan: &mut Plan,
) -> Result<()> {
    let local = cfg.html_path(date);

    match std::fs::read(&local) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            plan.write.insert(path);
        }
        Err(error) => {
            return Err(anyhow::Error::new(error))
                .with_context(|| format!("reading {}", local.display()));
        }
        Ok(existing) if sha256(&existing) == digest => plan.identical += 1,
        Ok(_) if force => {
            plan.write.insert(path);
        }
        Ok(_) => plan.conflicted.push(date),
    }

    Ok(())
}

fn restore(cfg: &Config, contents: &Contents, plan: &Plan) -> Result<usize> {
    let bar = progress::bar("restoring", plan.write.len());
    let mut written = 0;

    for (path, body) in contents {
        if !plan.write.contains(path) {
            continue;
        }

        let date = entry_date(path).expect("the inspect pass rejects anything else");
        bar.set_message(date.to_string());
        write_atomically(&cfg.html_path(date), body)?;
        written += 1;
        bar.inc(1);
    }

    bar.finish_and_clear();
    Ok(written)
}

fn unpack(snapshot: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    const ZSTD: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
    const ZIP: [u8; 2] = *b"PK";

    if snapshot.starts_with(&ZSTD) {
        return unpack_tar_zst(snapshot);
    }
    if snapshot.starts_with(&ZIP) {
        return unpack_zip(snapshot);
    }

    bail!("this is neither a zstd tarball nor a zip, so it is not a legacy snapshot")
}

fn unpack_tar_zst(snapshot: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let decoder = zstd::Decoder::new(snapshot).context("decompressing the snapshot")?;
    let mut archive = tar::Archive::new(decoder);
    let mut contents = Vec::new();

    for entry in archive.entries().context("reading the snapshot")? {
        let mut entry = entry.context("reading the snapshot")?;
        if entry.header().entry_type().is_dir() {
            continue;
        }

        let path = entry_path(&entry)?;
        ensure!(
            entry.header().entry_type().is_file(),
            "{path} is not a regular file, so this is not a legacy snapshot"
        );

        let mut body = Vec::new();
        entry
            .read_to_end(&mut body)
            .with_context(|| format!("reading {path} out of the snapshot"))?;
        contents.push((path, body));
    }

    Ok(contents)
}

fn unpack_zip(snapshot: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(snapshot))
        .context("opening the snapshot as a zip")?;
    let mut contents = Vec::new();

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).context("reading the snapshot")?;
        if file.is_dir() {
            continue;
        }

        let path = file
            .enclosed_name()
            .context("a snapshot entry has a path that escapes the archive")?
            .to_str()
            .context("a snapshot entry has a path that is not UTF-8")?
            .to_owned();

        let mut body = Vec::new();
        file.read_to_end(&mut body)
            .with_context(|| format!("reading {path} out of the snapshot"))?;
        contents.push((path, body));
    }

    Ok(contents)
}

fn entry_path<R: Read>(entry: &tar::Entry<'_, R>) -> Result<String> {
    let path = entry
        .path()
        .context("a snapshot entry has no readable path")?;
    let path = path
        .to_str()
        .context("a snapshot entry has a path that is not UTF-8")?;

    Ok(path.trim_start_matches("./").to_owned())
}

fn tar_path(date: ApodDate) -> String {
    format!("{HTML_ROOT}/{}", date.html_path())
}

fn entry_date(path: &str) -> Option<ApodDate> {
    let mut parts = path.strip_prefix(HTML_ROOT)?.strip_prefix('/')?.split('/');
    let (year, month, file) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }

    let date: ApodDate = file.strip_suffix(".html")?.parse().ok()?;
    (date.format("%Y") == year && date.format("%m") == month).then_some(date)
}

fn named(path: &str) -> String {
    entry_date(path).map_or_else(|| path.to_owned(), |date| date.to_string())
}

fn render_fetches(rows: &[FetchRow]) -> Result<String> {
    let mut out = String::new();
    for row in rows {
        out.push_str(&serde_json::to_string(row).context("rendering fetch state")?);
        out.push('\n');
    }
    Ok(out)
}

fn parse_fetches(body: &[u8]) -> Result<Vec<FetchRow>> {
    let text = std::str::from_utf8(body).context("fetches.jsonl is not UTF-8")?;

    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(n, line)| {
            serde_json::from_str(line).with_context(|| format!("fetches.jsonl line {}", n + 1))
        })
        .collect()
}

fn render_manifest(entries: &BTreeMap<String, String>) -> String {
    entries
        .iter()
        .map(|(path, digest)| format!("{digest}  {path}\n"))
        .collect()
}

fn parse_manifest(text: &str) -> Result<BTreeMap<String, String>> {
    let mut entries = BTreeMap::new();

    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let (digest, rest) = line
            .split_at_checked(64)
            .with_context(|| format!("manifest.sha256 line {} is too short to be one", n + 1))?;
        ensure!(
            digest.chars().all(|c| c.is_ascii_hexdigit()),
            "manifest.sha256 line {} does not start with a digest",
            n + 1
        );

        let path = rest.trim_start().trim_start_matches('*');
        ensure!(
            !path.is_empty(),
            "manifest.sha256 line {} names no file",
            n + 1
        );

        entries.insert(path.to_owned(), digest.to_ascii_lowercase());
    }

    ensure!(!entries.is_empty(), "manifest.sha256 lists nothing");
    Ok(entries)
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> ApodDate {
        ApodDate::from_ymd(y, m, d).unwrap()
    }

    fn config(root: &Path) -> Config {
        let mut cfg = Config::from_env().unwrap();
        cfg.html_dir = root.join("html");
        cfg.archive_db = root.join("archive.db");
        cfg
    }

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("apod-legacy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    async fn corpus(root: &Path, dates: &[ApodDate]) -> Config {
        let cfg = config(root);
        let archive = ArchiveStore::open(&cfg.archive_db).await.unwrap();

        for (n, &date) in dates.iter().enumerate() {
            let body = format!("<html>{date} page</html>");
            write_atomically(&cfg.html_path(date), body.as_bytes()).unwrap();
            archive
                .record_success(
                    date,
                    Source::Legacy,
                    &cfg.page_url(date),
                    &sha256(body.as_bytes()),
                    body.len(),
                    1_700_000_000 + n as i64,
                )
                .await
                .unwrap();
        }

        cfg
    }

    fn find(out: &Path, suffix: &str) -> String {
        std::fs::read_dir(out)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.to_str().is_some_and(|p| p.ends_with(suffix)))
            .unwrap_or_else(|| panic!("the export writes a {suffix}"))
            .to_str()
            .unwrap()
            .to_owned()
    }

    fn snapshot(out: &Path) -> Vec<u8> {
        std::fs::read(find(out, ".tar.zst")).unwrap()
    }

    fn repack(snapshot: &[u8], damage: impl Fn(&str, &mut Vec<u8>)) -> Vec<u8> {
        let mut contents = unpack(snapshot).unwrap();
        for (path, body) in &mut contents {
            damage(path, body);
        }
        pack_tar_zst(&contents).unwrap()
    }

    #[test]
    fn only_a_legacy_html_path_names_a_date() {
        assert_eq!(
            entry_date("html/2024/03/2024-03-05.html"),
            Some(date(2024, 3, 5))
        );
        assert_eq!(entry_date("html/2024/04/2024-03-05.html"), None);
        assert_eq!(entry_date("html/2024/03/2024-03-05.html.tmp"), None);
        assert_eq!(entry_date("../etc/passwd"), None);
        assert_eq!(entry_date("html/../../etc/passwd"), None);
        assert_eq!(entry_date(FETCHES), None);
    }

    #[test]
    fn a_manifest_round_trips_through_sha256sum_format() {
        let mut entries = BTreeMap::new();
        entries.insert("html/2024/03/2024-03-05.html".to_owned(), sha256(b"a"));
        entries.insert(FETCHES.to_owned(), sha256(b"b"));

        let rendered = render_manifest(&entries);
        assert!(rendered.contains(&format!("{}  {FETCHES}\n", sha256(b"b"))));
        assert_eq!(parse_manifest(&rendered).unwrap(), entries);
    }

    #[tokio::test]
    async fn a_snapshot_restores_the_tree_and_the_fetch_state() {
        let root = scratch("roundtrip");
        let dates = [date(1995, 6, 16), date(2024, 3, 5), date(2026, 8, 25)];
        let cfg = corpus(&root.join("source"), &dates).await;

        let out = root.join("out");
        export(&cfg, &out).await.unwrap();

        let manifest =
            parse_manifest(&std::fs::read_to_string(out.join(MANIFEST)).unwrap()).unwrap();
        assert_eq!(
            manifest.len(),
            dates.len() + 2,
            "every page plus fetches.jsonl and README.md"
        );

        let restored = config(&root.join("restore"));
        let plan = inspect(&restored, &unpack(&snapshot(&out)).unwrap(), false).unwrap();
        assert_eq!(plan.write.len(), dates.len());
        assert_eq!(plan.identical, 0);
        assert_eq!(plan.fetches.len(), dates.len());

        import(&restored, Some(find(&out, ".tar.zst")), None, false)
            .await
            .unwrap();

        for date in dates {
            assert_eq!(
                std::fs::read(restored.html_path(date)).unwrap(),
                std::fs::read(cfg.html_path(date)).unwrap(),
                "{date} did not come back byte for byte"
            );
        }

        let archive = ArchiveStore::open(&restored.archive_db).await.unwrap();
        let rows = archive.fetch_rows(Source::Legacy).await.unwrap();
        assert_eq!(rows.len(), dates.len());
        assert_eq!(
            rows[0].sha256,
            Some(sha256(b"<html>1995-06-16 page</html>"))
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn a_second_import_writes_nothing_and_seeds_nothing() {
        let root = scratch("idempotent");
        let dates = [date(2024, 3, 5), date(2024, 3, 6)];
        let cfg = corpus(&root.join("source"), &dates).await;

        let out = root.join("out");
        export(&cfg, &out).await.unwrap();

        let restored = config(&root.join("restore"));
        import(&restored, Some(find(&out, ".tar.zst")), None, false)
            .await
            .unwrap();

        let plan = inspect(&restored, &unpack(&snapshot(&out)).unwrap(), false).unwrap();
        assert!(plan.write.is_empty(), "nothing is left to write");
        assert_eq!(plan.identical, dates.len());
        assert!(plan.conflicted.is_empty());

        let archive = ArchiveStore::open(&restored.archive_db).await.unwrap();
        assert_eq!(archive.seed(&plan.fetches).await.unwrap(), 0);

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn a_local_page_that_differs_is_named_and_left_alone() {
        let root = scratch("conflict");
        let target = date(2024, 3, 5);
        let cfg = corpus(&root.join("source"), &[target]).await;

        let out = root.join("out");
        export(&cfg, &out).await.unwrap();

        let restored = config(&root.join("restore"));
        write_atomically(&restored.html_path(target), b"a different page").unwrap();

        let refused = import(&restored, Some(find(&out, ".tar.zst")), None, false)
            .await
            .unwrap_err()
            .to_string();
        assert!(refused.contains("--force"), "{refused}");
        assert!(refused.contains("1 date;"), "{refused}");
        assert_eq!(
            std::fs::read(restored.html_path(target)).unwrap(),
            b"a different page",
            "the local page must survive"
        );

        import(&restored, Some(find(&out, ".tar.zst")), None, true)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(restored.html_path(target)).unwrap(),
            std::fs::read(cfg.html_path(target)).unwrap()
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn a_damaged_file_fails_by_name_before_anything_is_written() {
        let root = scratch("damaged");
        let target = date(2024, 3, 5);
        let cfg = corpus(&root.join("source"), &[target]).await;

        let out = root.join("out");
        export(&cfg, &out).await.unwrap();

        let damaged = repack(&snapshot(&out), |path, body| {
            if entry_date(path).is_some() {
                body.extend_from_slice(b"<!-- rot -->");
            }
        });

        let restored = config(&root.join("restore"));
        let refused = inspect(&restored, &unpack(&damaged).unwrap(), false)
            .unwrap_err()
            .to_string();

        assert!(refused.contains("2024-03-05"), "{refused}");
        assert!(refused.contains("does not match the manifest"), "{refused}");
        assert!(!restored.html_path(target).exists(), "nothing was written");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn a_file_missing_from_the_snapshot_is_a_named_failure() {
        let root = scratch("missing");
        let dates = [date(2024, 3, 5), date(2024, 3, 6)];
        let cfg = corpus(&root.join("source"), &dates).await;

        let out = root.join("out");
        export(&cfg, &out).await.unwrap();

        let mut contents = unpack(&snapshot(&out)).unwrap();
        contents.retain(|(path, _)| entry_date(path) != Some(dates[1]));
        let trimmed = pack_tar_zst(&contents).unwrap();

        let restored = config(&root.join("restore"));
        let refused = inspect(&restored, &unpack(&trimmed).unwrap(), false)
            .unwrap_err()
            .to_string();
        assert!(refused.contains("2024-03-06"), "{refused}");
        assert!(refused.contains("missing from the snapshot"), "{refused}");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn the_snapshot_says_what_it_is_and_publishes_its_own_hash() {
        let root = scratch("selfdescribing");
        let dates = [date(1995, 6, 16), date(2024, 3, 5)];
        let cfg = corpus(&root.join("source"), &dates).await;

        let out = root.join("out");
        export(&cfg, &out).await.unwrap();

        let packed = snapshot(&out);
        let sums = parse_manifest(&std::fs::read_to_string(out.join(SUMS)).unwrap()).unwrap();
        let name = std::path::Path::new(&find(&out, ".tar.zst"))
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        assert_eq!(sums.get(&name), Some(&sha256(&packed)));
        assert_eq!(
            sums.get(MANIFEST),
            Some(&sha256(
                std::fs::read(out.join(MANIFEST)).unwrap().as_slice()
            ))
        );

        let contents = unpack(&packed).unwrap();
        let readme = String::from_utf8(entry(&contents, README).unwrap().to_vec()).unwrap();

        assert!(readme.contains("1995-06-16 to 2024-03-05"), "{readme}");
        assert!(readme.contains("sha256sum -c manifest.sha256"), "{readme}");
        assert!(readme.contains("Nemiroff"), "{readme}");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn a_snapshot_that_hashes_to_something_else_is_refused_unread() {
        let root = scratch("pinned");
        let target = date(2024, 3, 5);
        let cfg = corpus(&root.join("source"), &[target]).await;

        let out = root.join("out");
        export(&cfg, &out).await.unwrap();

        let restored = config(&root.join("restore"));
        let wrong = "0".repeat(64);
        let refused = import(&restored, Some(find(&out, ".tar.zst")), Some(wrong), false)
            .await
            .unwrap_err()
            .to_string();

        assert!(refused.contains("nothing has been read"), "{refused}");
        assert!(!restored.html_path(target).exists());

        let right = sha256(&snapshot(&out));
        import(&restored, Some(find(&out, ".tar.zst")), Some(right), false)
            .await
            .unwrap();
        assert!(restored.html_path(target).exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn the_zip_and_the_tarball_hold_the_same_archive() {
        let root = scratch("bothforms");
        let dates = [date(1995, 6, 16), date(2024, 3, 5)];
        let cfg = corpus(&root.join("source"), &dates).await;

        let out = root.join("out");
        export(&cfg, &out).await.unwrap();

        let zipped = std::fs::read(find(&out, ".zip")).unwrap();
        assert_eq!(
            unpack(&zipped).unwrap(),
            unpack(&snapshot(&out)).unwrap(),
            "both published forms have to restore the same archive"
        );

        let restored = config(&root.join("restore"));
        import(&restored, Some(find(&out, ".zip")), None, false)
            .await
            .unwrap();

        for date in dates {
            assert_eq!(
                std::fs::read(restored.html_path(date)).unwrap(),
                std::fs::read(cfg.html_path(date)).unwrap()
            );
        }

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn something_that_is_neither_form_is_refused() {
        let refused = unpack(b"<html>not an archive</html>")
            .unwrap_err()
            .to_string();
        assert!(refused.contains("not a legacy snapshot"), "{refused}");
    }

    #[tokio::test]
    async fn the_same_corpus_always_packs_to_the_same_bytes() {
        let root = scratch("deterministic");
        let cfg = corpus(&root.join("source"), &[date(2024, 3, 5)]).await;

        export(&cfg, &root.join("a")).await.unwrap();
        export(&cfg, &root.join("b")).await.unwrap();

        assert_eq!(snapshot(&root.join("a")), snapshot(&root.join("b")));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_import_with_nowhere_to_look_names_the_variable() {
        let root = scratch("unset");
        let mut cfg = config(&root);
        cfg.legacy_archive_url = None;

        let refused = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(obtain(&cfg, None))
            .unwrap_err()
            .to_string();

        assert!(refused.contains("APOD_LEGACY_ARCHIVE_URL"), "{refused}");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
