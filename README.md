# lemon-apod

A service for archiving and serving NASAs Astronomy Picture Of the Day in a modernized and improved way.

## What it does

- **Ability to archive every entry since 1995-06-16**, with the original explanation, its inline links and its
  attribution intact.
- **Search** across titles, explanations, credits and keywords, with phrases, exclusions and prefixes. Opening a result
  highlights the terms in the entry.
- **Browsing** by year, by month, as a calendar, as an endless feed, or at random. Favorites and read state live in the
  browser: there are no accounts and no server-side user state. Read progress is shown for whatever month or year you
  are looking at, and the settings allow you to backup your data.
- **A resource catalogue** of everything the archive links out to.
- **Statistics** over the whole archive and how it has changed year by year, down to how often every individual word has
  ever been used.

## Running it in containers

[docker/compose.yaml](docker/compose.yaml) runs both services against a shared `./data`, which is also the production
deployment example.

```bash
make up # build and start both services on :51995
```

## Running it locally

```bash
make backfill N=5
```

That fetches the five most recent entries into `./data`, parses them and generates thumbnails.
**It hits NASA's servers.** The default delay is 10 to 30 seconds between pages, and please leave
it that way.

```bash
make api
```

Serves the API and the built frontend on <http://localhost:51995>. For frontend work, run the API
in one terminal and `make dev` in another; Vite proxies `/api` and `/thumbs` across.

## Data layout

Everything lives under `APOD_DATA_DIR` (`/data` in the container, `./data` locally):

```
html/YYYY/MM/YYYY-MM-DD.html   raw bytes, exactly as served
thumbs/YYYY/MM/YYYY-MM-DD.webp grid thumbnails
archive.db                     fetch state, NOT derived, do not delete
apod.db                        parsed entries, search index, catalogue and word counts
```

The raw HTML on disk is the source of truth. `apod.db` is derived from it and can be deleted and
rebuilt with `make reparse`, without a single page being refetched. A rebuilt index has forgotten where the thumbnails
are, so follow it with `make thumbs`, which adopts every file already on disk and only fetches the ones genuinely
missing.

`archive.db` records when each page was fetched and what it hashed to, which is not recoverable from the filesystem.
Losing it means re-fetching the whole archive at whatever delay configured.
