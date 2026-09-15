# lemon-apod

A service for archiving and serving NASA's Astronomy Picture of the Day in a modernized and improved way.

## What it does

- **Ability to archive every entry since 1995-06-16**, with the original explanation, its inline links and its
  attribution intact.
- **Search** across titles, explanations, credits and keywords, with phrases, exclusions and prefixes. Opening a result
  highlights the terms in the entry.
- **Browsing** by year, by month, as a calendar, as an endless feed, or at random. On the website, favorites and read
  state live in the browser: there are no accounts and no server-side user state. The Discord bot keeps its own
  favorites in `bot.db`, one row per person per entry. Read progress is shown for whatever month or year you are
  looking at, and the settings allow you to backup your data.
- **A resource catalogue** of everything the archive links out to.
- **A credit index and an object index.** Every party an entry credits, gathered so that one photographer is one
  photographer however many ways their name has been printed. And what the entries say they are pictures of, read from
  catalogue designations, the bodies of the solar system, the constellations, named stars, meteor showers and a table of
  common names, told apart by whether the picture is *of* the object or merely mentions it. Both are browsable from
  `/indexes`, alongside the encores and the resource catalogue.
- **Statistics** over the whole archive and how it has changed year by year, down to how often every individual word has
  ever been used.
- **Four games** over the archive: guess when a blurred picture ran, uncover a redacted explanation one word at a time,
  put two pictures in chronological order, or match an explanation to the picture it describes. Each one deals the same
  puzzle to everybody each day (but also offers a free play mode).
- **What the sky is up to**, on the front page: the moon's phase, which planets are up and when, the next meteor shower
  and whether the moon will ruin it, the next eclipse, and the next few rocket launches. `/sky` carries all of it for
  any date between 1900 and 2100, and links each kind of event to the entries that caught it. `/launches` carries every
  launch coming up and the last thirty days of them.
- **Feeds and push notifications**. The latest entries with their explanations as Atom at `/atom.xml` or RSS at
  `/feed.xml`, both linked from the page head so a reader can find them on its own. Optionally, four
  [ntfy](https://ntfy.sh) topics: the picture of the day, aurora alerts, the rest of NOAA's space weather, and sky
  events a day or so ahead. See [/notifications](https://apod.lemon.industries/notifications) for how to subscribe.
- **A Discord bot**. Announces each new entry in whichever channel a server picks, sends it as a direct message to
  anyone who asks for it, and answers `/apod today`, `/apod date`, `/apod random`, `/apod on-this-day` and
  `/apod search`. A server is pointed at a channel with `/apod settings`, `/apod announce` force-posts today's entry,
  and `/apod dm` starts or stops the direct messages. Every card carries a favorite button that anyone who can see it
  may press, and `/apod favorites` lists what you have kept. It reads the same archive as the API rather than calling
  it.

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
apod.db                        parsed entries, search index, catalogue, word counts, picture hashes,
                               credit and object indexes
sky.db                         upcoming launches and space weather, refetched hourly
notify.db                      which notifications have already gone out
bot.db                         Discord server settings, how far each has been told, and favorites
```

The raw HTML on disk is the source of truth. `apod.db` is derived from it and can be deleted and
rebuilt with `make reparse`, without a single page being refetched. A rebuilt index has forgotten where the thumbnails
are, so follow it with `make thumbs`, which adopts every file already on disk and only fetches the ones genuinely
missing.

The reference tables in [core/tables](core/tables) are the other half of what a reparse reads: which catalogues exist
and how far each counts, what the Moon is called, that "the Orion Nebula" means M42, and which two spellings of an
institute are one institute. They are data rather than parser logic, so they live in CSV files where a diff reads as a
change to the list. They are compiled in, so editing one is a rebuild and a `make reparse`, which `PARSER_VERSION`
makes the archive ask for.

`archive.db` records when each page was fetched and what it hashed to, which is not recoverable from the filesystem.
Losing it means re-fetching the whole archive at whatever delay configured.

`bot.db` is the Discord bot's own, and the only file it writes. One row per server holds the channel, the optional
message, the explanation length and the newest entry that server has already been told about, and one row per person
holds the same for a direct message subscription. Favorites live here too, one row per person per entry, and a person
can throw their own away with `/apod favorites clear:true`. Deleting it forgets every server's setup, everyone's
subscription and every favorite, and each has to be set again with `/apod settings` or `/apod dm`. The API opens the
same file read only, to publish a few counts on its Discord page; the file is optional, and an API running without a
bot beside it shows none of them.

`sky.db` holds the two things the front page cannot work out for itself, rocket launches and the current geomagnetic
activity. The archiver refreshes it every half hour by default (`APOD_SKY_INTERVAL_SECS`), and drops to
`APOD_SKY_IMMINENT_INTERVAL_SECS` once a launch is within `APOD_SKY_IMMINENT_HOURS` or already streaming, because a
hold or a scrub reaches the feed within minutes. `make sky` does one pass now and prints what it got. Launches serve
from their own short `APOD_CACHE_LAUNCHES_SECS`, which is why `/api/sky` no longer carries them. Deleting the file costs
one poll, plus whatever launch history had built up: launches are kept for `APOD_SKY_LAUNCH_HISTORY_DAYS` after they
fly, because the upcoming feed stops listing one the moment it goes up. Webcast links come from the detailed feed, which
runs to tens of kilobytes a launch, so it is asked only about the next `APOD_SKY_WEBCAST_LOOKAHEAD`. Everything else on
those pages, the moon and the planets and the showers and the eclipses and the conjunctions, is computed on the spot and
needs neither this file nor a network, for any date between 1900 and 2100.

`notify.db` records every notification already sent, which is what stops the same eclipse being announced on every
pass. Deleting it re-announces everything currently inside its lead window, so on an archive that has been running a
while, seed it instead:

```bash
make notify SEED=1
```

`make notify DRY=1` lists what is due, with the link each message would open, and touches nothing.

### When APOD stops publishing

Government shutdowns have twice taken APOD's site down for weeks. Both times the missing days were published afterwards
with their original dates, so the archive has no holes from either, but an archiver watching it happen writes those days
off as permanently absent and never asks again unless it is told.

Set `APOD_PAUSE_START` to the first day APOD went quiet, as `YYYY-MM-DD`, read in `APOD_PUBLISH_TZ`. `APOD_PAUSE_END` is
optional and closes the window; `APOD_PAUSE_REASON` is free text for the notice on the front page. Both services read
all three. While the window runs the workers stand down, and every date it covers that has no entry is treated as one
never asked for, so the backfill picks those days up the moment APOD resumes.

**Leave `APOD_PAUSE_START` set until the backfill has caught up.** Dropping it while those days are still empty settles
their 404s again, which is what the flag exists to prevent.
