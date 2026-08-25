# Streamable dashboard More menu: Download links

Implementation status: **Not implemented in `streamable-rs`.**

Captured 2026-08-13 from the authenticated Streamable dashboard, production
source maps, Chrome DevTools network inspection, and comparison with the
sibling Python client. This file records only the **Download links** feature
and its resolution listing, direct-download, stable MP4 URL, processing,
plan-gating, and upgrade branches. Cookies, signed query parameters, concrete
shortcodes, account data, and telemetry are intentionally omitted.

## Entry and data source

**More > Download links** opens a local modal titled `Download`. Opening and
closing it makes no Streamable API request. The modal renders from the complete
video representation already cached by the dashboard, specifically:

```text
video.files
video.available_file_resolutions
video.shortcode
me.plan
me.no_trial
```

The available file order is fixed:

```text
mp4-high
mp4
mp4-mobile
```

The UI maps presets to labels as follows:

| Preset       | Normal label | Special case                            |
| ------------ | ------------ | --------------------------------------- |
| `mp4-high`   | Ultra HD     | none                                    |
| `mp4`        | HD           | relabeled SD when height is at most 480 |
| `mp4-mobile` | SD           | none                                    |

A ready file's description is computed locally:

```text
(<width> × <height> / <size divided by 1,000,000, fixed to one decimal>MB)
```

The live version-2 video had one ready `mp4` file at 640x360 and 3,632,116
bytes, so the modal displayed:

```text
SD (640 × 360 / 3.6MB)
Download
MP4 URL
```

No HEAD, metadata, or link-generation request is made when the row appears.

## Available, disabled, and processing rows

The modal first compares `available_file_resolutions` with actual keys in
`video.files`.

Resolutions named as available but missing from `files` are rendered before
real files as disabled rows. They use a placeholder height of 2160, show a
`BASIC` star/upgrade indicator, omit the normal size description, and disable
both Download and MP4 URL buttons. Clicking the upgrade indicator dispatches
the source `file-download-list` to the dashboard upgrade flow.

For a real file:

- status `2` means ready;
- any other status omits both actions and displays `Processing...`;
- a ready file uses the exact `file.url` returned in the video representation
  as its direct download URL.

The modal does not poll independently. Processing state changes only when the
dashboard's normal video polling updates the cached video.

## Direct Download network flow

The Download button is available for a ready file even when the account has no
paid plan. It creates a temporary anchor:

```text
href = file.url
download = final slash-separated segment of file.url
```

The browser appends the anchor to the document, clicks it, and removes it. The
frontend does not proxy the bytes through a Streamable JSON API.

The live click produced:

```http
GET https://cdn-cf-east.streamable.com/video/mp4/<shortcode>_2.mp4?<signed-query-redacted>

HTTP/1.1 200
Content-Type: video/mp4
Content-Length: 3632116
Content-Disposition: attachment;
Accept-Ranges: bytes
Cache-Control: max-age=315360000
```

The request had no body. The response contained the exact MP4 bytes.

The CDN path included the active replacement version suffix `_2`. The opaque
`url` returned for the file is authoritative; construction from `version` is not
part of the observed contract. Expiry, key-pair, and signature query values are
ephemeral.

Although the observed browser request was HTTP 200, the server advertises byte
ranges. HTTP 200 and 206 are both compatible with this behavior, and the body is
suitable for streaming rather than buffering as one large value.

## MP4 URL behavior

MP4 URL does not copy the signed CDN URL. The frontend constructs a stable
Streamable link from the shortcode and preset:

```text
https://streamable.com/l/<shortcode>/<preset>.mp4
```

Examples of the final path segment are therefore:

```text
mp4-high.mp4
mp4.mp4
mp4-mobile.mp4
```

The row's `SD` relabeling for a low-height `mp4` file does not change its stable
link preset; it remains `/mp4.mp4`.

Plan gating uses JavaScript truthiness of `me.plan`:

- any truthy plan: write the stable URL with `navigator.clipboard.writeText`;
- no plan: the clipboard remains unchanged and an upgrade modal opens.

On successful clipboard write, button text changes from `MP4 URL` to `Copied`
for three seconds. If the Clipboard API is unavailable, the helper logs
`Clipboard API not available`; it has no legacy selection fallback and does
not mark the operation as copied through another mechanism.

The free-account live click opened a nested modal with:

```text
Streamable Basic Required
Upgrade now to access MP4 video URLs.
```

Its internal source is `mp4url`. Merely opening this modal made no file request
and did not navigate away from the dashboard. Trial behavior uses the
`noFreeTrials` experiment when enabled; otherwise it uses `me.no_trial`.

Thus the current product distinction is intentional:

- direct download of an already-ready signed file is allowed on the free
  account;
- copying a stable, non-expiring MP4 URL requires a truthy paid plan.

## Live verification summary

The authenticated ready video exercised these branches:

1. opening Download links made no API request and rendered cached version-2
   file metadata;
2. its 640x360 `mp4` file was relabeled `SD` and displayed as 3.6MB;
3. free-account MP4 URL opened the Basic upsell without a clipboard write or
   file request;
4. Download issued one signed CDN GET and received HTTP 200 with 3,632,116
   video bytes and attachment disposition;
5. the underlying video, version, privacy, labels, and captions were not
   mutated.

No downloaded bytes, signed URL, concrete shortcode, clipboard content, or
account field is durable project state.

## Python parity

The sibling Python implementation does not expose video file representations,
direct download, or stable MP4 URL operations.
