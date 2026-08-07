# lemon-apod

A service for archiving and serving NASAs Astronomy Picture Of the Day in a modernized and improved way.

Thirty years of APOD, kept as the raw HTML it was published as, parsed into structured entries, and
served as a site you can actually read, search and wander through. Alongside the entries it builds a
catalogue of every resource the explanations link to, and counts every word they were written with.

```
core/       apod-core     parsing, shared types, date maths, and the apod.db schema
archiver/   apod-archiver fetch + parse + thumbnails, as a service and a CLI
api/        apod-api      axum: JSON API, static frontend, Open Graph injection
web/                      Vue 3 + Vite + TypeScript + PrimeVue
docker/     Dockerfile    two targets: archiver, api
            compose.yaml  both of them, sharing one ./data
```

## What it does

- **Every entry since 1995-06-16**, with the original explanation, its inline links and its
  attribution intact. Cross-references between entries are followed inside the site rather than
  bouncing out to NASA.
- **Search** across titles, explanations, credits and keywords, with phrases, exclusions and
  prefixes. Opening a result highlights the terms in the entry.
- **Browsing** by year, by month, as a calendar, as an endless feed, or at random. Favorites and
  read state live in the browser: there are no accounts and no server-side user state.
- **A resource catalogue** of everything the archive links out to, searchable and filterable by
  site, ranked by how often it has been referenced.
- **Statistics** over the whole archive and how it has changed year by year, down to how often
  every individual word has ever been used.

## Running it in containers

[docker/compose.yaml](docker/compose.yaml) runs both services against a shared `./data`, and is
also the deployment example. The production differences are commented inline.

```bash
make up      # build and start both services on :51995
```

That works against an empty `./data`, because the archiver migrates both databases at startup and
the API waits for it. Running `make seed` first is optional, and only fetches a few pages so the
site has something to show immediately. `./data` is a bind mount, so it survives `make down`.

`make logs` follows both services, `make ps` shows health, and the archiver CLI is reachable in the
running container:

```bash
docker compose -f docker/compose.yaml exec archiver apod-archiver status
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
rebuilt with `make reparse` in minutes, without a single page being fetched again. A rebuilt index
has forgotten where the thumbnails are, so follow it with `make thumbs`, which adopts every file
already on disk and only fetches the ones genuinely missing.

`archive.db` records when each page was fetched and what it hashed to, which is not recoverable
from the filesystem. Losing it means re-fetching the whole archive at roughly 20 seconds per page.

**One writer at a time.** Do not run the `make` targets against `./data` while the containers are
up. SQLite's file locking does not carry across a Docker Desktop bind mount, so a host-side
`reparse` and a containerised archiver will both believe they hold the write lock and corrupt
`apod.db` between them. Either stop the stack first, or go through the container:

```bash
docker compose -f docker/compose.yaml exec archiver apod-archiver reparse
```

If it does happen, the symptom is `database disk image is malformed` and the fix is to delete
`apod.db` and reparse. `archive.db` is only written when a page is actually fetched, so it is
rarely in the line of fire.

## Archiver commands

```bash
apod-archiver run                    # the service: backfill, daily poll, re-check
apod-archiver backfill [--limit N]   # fetch missing pages now, newest first
apod-archiver fetch <date> [--force]
apod-archiver reparse [--stale] [--from D] [--to D]
apod-archiver thumbs [--force] [--limit N]
apod-archiver quality [--date D] [--warning KIND] [--limit N]
apod-archiver status
```

## Configuration

Everything is environment variables with working defaults. See `Config::from_env` in
[archiver/src/config.rs](archiver/src/config.rs) and [api/src/config.rs](api/src/config.rs).

## Licence

MIT. Note that this covers the code only: the archived explanations come from NASA, and the images
they describe belong to the photographers and institutions credited on each entry.
