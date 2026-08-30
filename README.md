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

## The Discord bot

`apod-discord-bot` is a third service over the same `./data`. It opens `apod.db` read-only, reads thumbnails straight
off the thumbnail root, and owns nothing but `bot.db`.

A server administrator sets it up with one command:

```
/apod settings announce:true channel:#space message:<@&123> explanation:Full explanation
```

`message` is the line posted above the embed, which is how a server pings a role; leave it out for no line at all, or
send a single `-` to clear one. `explanation` chooses between NASA's full text, a short teaser and none at all.
Everything except the channel is optional, and running `/apod settings` with nothing set just reports where the server
stands.

`/apod announce` posts today's entry into that channel immediately, exactly as the daily one arrives, mention included.
Run in a direct message it does the same thing there, which is also the way to check Discord will actually deliver one
before relying on it.
It works before announcements are switched on, which is when you want to see it, and it counts as today's: it moves the
server's watermark, so the watcher does not come along later and post the same entry a second time. Run on an entry
that already went out, it says so and posts a second copy anyway.

The same application also installs to a person rather than a server, which carries the commands into their own DMs and
anywhere else they use Discord. That install takes no permissions and no bot scope at all, because there is no channel
for it to be granted anything in. `/apod dm subscribe:true` then delivers each new entry as a direct message.

Discord will only let a bot open a DM with somebody it shares a server with, and only if their privacy settings allow
it; otherwise the send comes back as
[50007](https://github.com/discord/discord-api-docs/issues/8238). So `/apod dm` sends the first entry there and then,
before saving anything: if it does not arrive, the person is told why on the spot rather than waiting for a message
that was never going to come. A subscriber who later closes their DMs is dropped on the next pass rather than retried
forever.

It asks for four permissions and no others: view the channel, send messages, embed links and attach files. That is
everything an embed with a picture needs. It deliberately does not ask to mention everyone, so a role ping only fires
if the role is set as mentionable, which is the server's own setting. The permission set is decided in
`api/src/config.rs` rather than in a pasted link, and the site builds the invite URL from `APOD_DISCORD_CLIENT_ID`.
Set that and the site grows a **Discord bot** page at `/discord` with the invite button on it; leave it unset and the
page is hidden.

Announcements are found by watching the archive rather than by a clock: every `APOD_BOT_POLL_SECS` (60 by default) the
bot looks at the newest entry, and posts it to any server whose watermark is behind it. An entry older than
`APOD_BOT_MAX_AGE_SECS` (36 hours) is never announced, so a server that turns the bot on today is not told about last
week, and a failed send simply retries on the next pass until that window closes.

To see what a post would look like without a token, a server, or anything reaching Discord:

```bash
make preview DATE=2026-08-30
```

Set `DISCORD_TOKEN` to run it, and `APOD_BOT_ANNOUNCE_ENABLED=false` to have it answer commands without posting
anything on its own, which is what a second instance against the same data wants.

The slash commands are not pushed to Discord on startup, because the list changes on a release rather than on a restart.
After deploying a version that adds or changes one, an owner mentions the bot:

```
@apod sync
```

That is the only thing the bot reads a message for, and it answers with buttons for registering or clearing the
commands globally or in one server. It is a prefix command by necessity: it is what registers the slash commands, so it
cannot be one itself. `APOD_BOT_OWNER_IDS` adds owners beyond whoever owns the Discord application, who always counts.

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

`sky.db` holds the two things the front page cannot work out for itself, upcoming rocket launches and the current
geomagnetic activity. The archiver refreshes it every half hour by default (`APOD_SKY_INTERVAL_SECS`), and `make sky`
does one pass now and prints what it got. Deleting it costs one poll. Everything else on those panels, the moon and the planets and the showers and the
eclipses, is computed on the spot and needs neither this file nor a network.

`notify.db` records every notification already sent, which is what stops the same eclipse being announced on every
pass. Deleting it re-announces everything currently inside its lead window, so on an archive that has been running a
while, seed it instead:

```bash
make notify SEED=1
```

`make notify DRY=1` lists what is due, with the link each message would open, and touches nothing.

`bot.db` is the Discord bot's own, and the only file it writes. One row per server holds the channel, the optional
message, the explanation length and the newest entry that server has already been told about, and one row per person
holds the same for a direct message subscription. Deleting it forgets every server's setup and everyone's subscription,
and each has to be set again with `/apod settings` or `/apod dm`.