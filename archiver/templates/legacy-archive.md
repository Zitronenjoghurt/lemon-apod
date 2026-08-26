# APOD legacy archive

A complete copy of NASA's Astronomy Picture of the Day as it was served from `apod.nasa.gov`. The site ran on from 1995
until its move to `science.nasa.gov/apod` in September 2026.

The legacy APOD entries were written and curated by Robert Nemiroff and Jerry Bonnell. This repository holds nothing but
a copy of the original HTML files.

This archive was created through a fully seeded instance of [Lemon-APOD](https://github.com/Zitronenjoghurt/lemon-apod).
New instances can easily import this archive to serve legacy APOD data.

## Archive contents

| | |
| --- | --- |
| Pages | {files} |
| Range | {first} to {last} |
| Gaps | {gaps} |
| Bytes | {bytes} |

```
html/YYYY/MM/YYYY-MM-DD.html   the page, byte for byte original
fetches.jsonl                  when each page was fetched, and its hash
manifest.sha256                a sha256 of every file above
README.md                      what this is, in the archive itself
```

## Verification

`SHA256SUMS` is attached to the release and covers the assets themselves, so a download can
be checked before it is opened:

```bash
sha256sum -c SHA256SUMS
```

Then, after unpacking, `manifest.sha256` covers the contents:

```bash
sha256sum -c manifest.sha256
```

Every line should read `OK`.

## Licence

NASA material is generally not subject to copyright, but individual APOD images are frequently credited to
photographers, observatories and institutions who do hold rights in them, and the credit line on each page is the
authority on that. Since this archive does not include any images and therefore only NASA content, it can be assumed to
be public domain (don't quote me on that though).