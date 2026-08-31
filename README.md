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
- **Four games** over the archive: guess when a blurred picture ran, uncover a redacted explanation one word at a time,
  put two pictures in chronological order, or match an explanation to the picture it describes. Each one deals the same
  puzzle to everybody each day (but also offers a free play mode).
- **What the sky is up to**, on the front page: the moon's phase, which planets are up and when, the next meteor shower
  and whether the moon will ruin it, the next eclipse, and the next few rocket launches.
- **Feeds and push notifications**. The latest entries with their explanations as Atom at `/atom.xml` or RSS at
  `/feed.xml`, both linked from the page head so a reader can find them on its own. Optionally, four
  [ntfy](https://ntfy.sh) topics: the picture of the day, aurora alerts, the rest of NOAA's space weather, and sky
  events a day or so ahead. See [/notifications](https://apod.lemon.industries/notifications) for how to subscribe.
- **A Discord bot**. Announces each new entry in whichever channel a server picks, sends it as a direct message to
  anyone who asks for it, and answers `/apod today`, `/apod date`, `/apod random` and `/apod search`. It reads the same
  archive as the API rather than calling it.

## Running it in containers

[docker/compose.yaml](docker/compose.yaml) runs the services against a shared `./data`, which is also the production
deployment example.

```bash
make up # build and start the archiver and the API on :51995
```

The bot sits behind a compose profile, because a real token posts into real servers:

```bash
DISCORD_TOKEN=... docker compose -f docker/compose.yaml --profile bot up -d
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
apod.db                        parsed entries, search index, catalogue, word counts, picture hashes
sky.db                         upcoming launches and space weather, refetched hourly
notify.db                      which notifications have already gone out
bot.db                         Discord server settings, and how far each has been told
```

The raw HTML on disk is the source of truth. `apod.db` is derived from it and can be deleted and
rebuilt with `make reparse`, without a single page being refetched. A rebuilt index has forgotten where the thumbnails
are, so follow it with `make thumbs`, which adopts every file already on disk and only fetches the ones genuinely
missing.

`archive.db` records when each page was fetched and what it hashed to, which is not recoverable from the filesystem.
Losing it means re-fetching the whole archive at whatever delay configured.

`bot.db` is the Discord bot's own, and the only file it writes. One row per server holds the channel, the optional
message, the explanation length and the newest entry that server has already been told about, and one row per person
holds the same for a direct message subscription. Deleting it forgets every server's setup and everyone's subscription,
and each has to be set again with `/apod settings` or `/apod dm`.

`sky.db` holds the two things the front page cannot work out for itself, upcoming rocket launches and the current
geomagnetic activity. The archiver refreshes it every half hour by default (`APOD_SKY_INTERVAL_SECS`), and `make sky`
does one pass now and prints what it got. Deleting it costs one poll. Everything else on those panels, the moon and the
planets and the showers and the
eclipses, is computed on the spot and needs neither this file nor a network.

`notify.db` records every notification already sent, which is what stops the same eclipse being announced on every
pass. Deleting it re-announces everything currently inside its lead window, so on an archive that has been running a
while, seed it instead:

```bash
make notify SEED=1
```

`make notify DRY=1` lists what is due, with the link each message would open, and touches nothing.