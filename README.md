# lemon-apod

A service for archiving and serving NASAs Astronomy Picture Of the Day in a modernized and improved way.

Three parts. The **archiver** slowly fetches the raw HTML and keeps it, the **core** library parses
that HTML into structured entries, and the **API** serves the parsed index and the frontend. The raw
HTML on disk is the source of truth. Everything else is derived and can be rebuilt from it without
touching the network, which is what makes it safe to keep improving the parser forever.

See [SPEC.md](SPEC.md) for the full design.

```
core/       apod-core     parsing, shared types, date maths, and the apod.db schema
archiver/   apod-archiver fetch + parse + thumbnails, as a service and a CLI
api/        apod-api      axum: JSON API, static frontend, Open Graph injection
web/                      Vue 3 + Vite + TypeScript + PrimeVue
docker/     Dockerfile    two targets: archiver, api
            compose.yaml  both of them, sharing one ./data
```

`apod.db` is shared, so its schema and queries live in `apod-core` behind feature gates rather
than in whichever binary got there first: `data` for the SQLite plumbing alone, `data-read` for
`ApodReader`, `data-write` for `ApodWriter`. A new consumer takes `data-read` for `apod.db` and
`data` for its own database. See [SPEC.md](SPEC.md#5-database) for the details.

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

## Running it in containers

[docker/compose.yaml](docker/compose.yaml) runs both services against a shared `./data`, and is
also the deployment example. The production differences are commented inline.

```bash
make up      # build and start both services on :51995
```

That works against an empty `./data`, because the archiver migrates both databases at startup and
the API waits for it. Running `make seed` first is optional, and only fetches a few pages so the
site has something to show immediately. `./data` is a bind mount, so it survives `make down`.

`make logs` follows both services, `make ps` shows health, and the archiver CLI is reachable in
the running container:

```bash
docker compose -f docker/compose.yaml exec archiver apod-archiver status
```

## Data layout

Everything lives under `APOD_DATA_DIR` (`/data` in the container, `./data` locally):

```
html/YYYY/MM/YYYY-MM-DD.html   raw bytes, exactly as served
thumbs/YYYY/MM/YYYY-MM-DD.webp grid thumbnails
archive.db                     fetch state, NOT derived, do not delete
apod.db                        parsed entries + search index, derived, safe to delete
```

`apod.db` can be deleted and rebuilt with `make reparse` in minutes, which is also how a schema
change is applied: there are no migrations, delete it and reparse. A rebuilt index has forgotten
where the thumbnails are, so follow it with `make thumbs`, which adopts every file already on disk
and only fetches the ones genuinely missing. `archive.db` records when each
page was fetched and what it hashed to, which is not recoverable from the filesystem. Losing it
means re-fetching the whole archive at roughly 20 seconds per page.

## Refining the parser

Thirty years of hand-written HTML does not parse perfectly on the first try, and it doesn't have to:

```bash
make quality    # entries with suspicious output, grouped by warning
# ...fix an extractor in core/src/parse/...
make reparse    # re-run it over every archived page
```

No page is ever fetched twice for this. When a parser change alters stored output, bump
`PARSER_VERSION` in `core/src/lib.rs` so `apod-archiver reparse --stale` can find the rows that
predate it.

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

In production these run through the container:

```bash
docker compose -f docker/compose.yaml exec archiver apod-archiver status
```

## Configuration

Everything is environment variables with working defaults. See `Config::from_env` in
[archiver/src/config.rs](archiver/src/config.rs) and [api/src/config.rs](api/src/config.rs), and
the tables in [SPEC.md](SPEC.md).

## Licence

MIT. Note that this covers the code only: the archived explanations come from NASA, and the images
they describe belong to the photographers and institutions credited on each entry.
