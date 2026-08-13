# Streamable dashboard More menu: Captions

Implementation status: **Not implemented in `streamable-rs`.**

Captured 2026-08-13 from the authenticated Streamable dashboard, production
source maps, Chrome DevTools network inspection, and comparison with the
sibling Python client. This file records only the flag-gated **Captions**
feature and its list, display, delete-confirmation, refetch, error, and currently
inert add-track branches. Cookies, concrete shortcodes, caption IDs, account
data, and telemetry are intentionally omitted.

## Feature-flagged menu availability

The video card includes **More > Captions** only when this boolean flag is true:

```text
expose-video-manage-captions
```

The live authenticated account returned:

```json
{ "expose-video-manage-captions": false }
```

Accordingly, Captions did not appear in its More menu. This is distinct from
labels: Captions visibility depends directly on the flag rather than whether
the video currently has caption records.

Source inspection still identifies the complete currently implemented menu
flow. The authenticated API endpoints were also exercised directly against the
disposable video to validate current wire behavior without forcing frontend
state or changing the feature flag.

## Open and list flow

When exposed, selecting Captions dispatches the current video shortcode and
opens a local modal titled `Captions`. A React effect keyed by shortcode then
sends:

```http
GET https://api-f.streamable.com/api/v1/videos/<shortcode>/captions
Content-Type: application/json
Pragma: no-cache
Cache-Control: no-cache
Cookie: <session; redacted>
```

The live direct request returned HTTP 200:

```json
{ "captions": [] }
```

The response envelope is therefore an object with a `captions` array, not a
bare array. The frontend replaces its modal list with that array.

While the GET is pending, the modal displays a spinner. Any request/parsing
failure is logged to the console and collapsed to the fixed visible message:

```text
Failed to fetch captions
```

The detailed server message is not displayed in the modal.

Closing the captions modal is local-only and does not clear the stored
shortcode or caption list. A later open with the same shortcode can therefore
briefly retain old list state until the fetch action updates it.

## Caption representation and display

The current UI consumes these caption fields:

```text
id: numeric deletion identifier
language: locale/language code
label: optional display label
```

Each row is keyed by `language` and its Delete action carries `id`.

Display-name behavior is unusual:

- if `label` is anything other than the exact string `Default`, the UI renders
  that label directly;
- only a `Default` label triggers `Intl.DisplayNames` translation of
  `language` into an English language name;
- if translation returns the original locale or throws, the code falls back to
  `label`, then to the locale when the label is nullish.

Because non-`Default` labels return before fallback, an absent label can render
blank rather than the language code. Duplicate language entries also produce
duplicate React keys. A future API model should preserve both server fields and
not treat this UI formatting as the wire contract.

## Add caption track is not implemented in this bundle

The modal always renders an `Add caption track` button, including when the list
is empty. In the inspected production source the button has no `onClick`, no
action, no file input, and no upload request implementation.

A complete search of the current source map found caption API calls only for:

```text
GET    /videos/<shortcode>/captions
DELETE /videos/<shortcode>/captions/<caption-id>
```

There is no source-backed POST/PUT endpoint in this bundle. Future Rust work
must not infer an upload path from the inert button or neighboring routes. Add
support only after a newer UI bundle or live enabled account provides wire
evidence.

## Delete confirmation flow

Clicking a row's Delete icon closes the list modal and opens a danger modal:

```text
Title: Delete captions
Body: Are you sure you would like to delete captions for this language?
Confirm: Delete captions
```

The confirmation state stores both shortcode and numeric caption ID. The
confirm callback dispatches only when the ID is truthy, so an unexpected ID 0
would currently do nothing.

Confirm sends a bodyless request:

```http
DELETE https://api-f.streamable.com/api/v1/videos/<shortcode>/captions/<caption-id>
Content-Type: text/plain
Pragma: no-cache
Cache-Control: no-cache
Cookie: <session; redacted>
```

The API helper is explicitly configured for text response handling. HTTP 204
is accepted before any body parsing. On success the UI:

1. closes the delete modal;
2. reopens the captions modal;
3. explicitly sends a new captions GET;
4. replaces the list with the returned array.

The explicit refetch is important because the reducer does not remove the
caption optimistically.

Cancel closes the delete modal and reopens the captions modal. It does not
explicitly refetch. Because the shortcode is normally unchanged, the list
component's shortcode-keyed effect need not run again.

While deletion is pending, the confirmation action is disabled. A failed
delete keeps the confirmation modal open and displays:

```text
Failed to delete captions
```

## Verified missing-caption error

The disposable video had no captions. A DELETE using a deliberately nonexistent
numeric caption ID safely exercised the failure response:

```http
DELETE /api/v1/videos/<shortcode>/captions/<missing-caption-id>
Content-Type: text/plain

HTTP/1.1 404
Content-Type: application/json; charset=utf-8

{"statusCode":404,"error":"Not Found","message":"Not Found"}
```

No video state changed, and a prior GET had proved the caption list empty.

There is a current frontend error-detail bug: because the delete helper requests
text handling, it reads the JSON error body as a string and then attempts
`payload.message`. The string has no `message` property, so the specific
`Not Found` message is discarded before the saga emits the generic visible
error. A future Rust implementation should parse structured errors when the
response content type is JSON even though successful deletion is bodyless.

## Live verification summary

The authenticated disposable video provided these observations:

1. account flag false kept Captions out of More;
2. direct authenticated GET returned HTTP 200 and exactly
   `{"captions":[]}`;
3. direct bodyless DELETE of a missing caption returned HTTP 404 with the
   structured Not Found JSON body;
4. the video remained ready at version 2 with zero captions;
5. no caption or other video resource was created or changed.

No concrete shortcode, caption ID, cookie, locale record, or account field is
durable project state.

## Python parity and future Rust API

The sibling Python implementation has no caption-list, caption-delete, or
caption-upload operation. Current verified Rust scope should therefore remain
limited to the two observed operations:

```text
list_video_captions(shortcode)
delete_video_caption(shortcode, caption_id)
```

Model list responses with a `captions` envelope and preserve unknown fields for
forward compatibility. Treat caption ID and language as independent; the
delete route uses ID, while language is only display metadata.

Do not add `upload_video_caption` from the present evidence. The visible add
button is inert, and no upload wire behavior exists in the inspected bundle.

## Deterministic test targets

Local mock coverage should verify:

1. listing uses GET on `/api/v1/videos/<shortcode>/captions`;
2. the authenticated cookie and cache-control headers are present;
3. `{"captions":[]}` and populated envelopes deserialize correctly;
4. caption ID, language, label, and unknown fields are preserved as designed;
5. deletion uses bodyless DELETE on
   `/api/v1/videos/<shortcode>/captions/<caption-id>`;
6. HTTP 204 succeeds without text or JSON decoding;
7. successful deletion can be followed by one list refetch;
8. JSON error bodies are parsed even when the success type is empty/text;
9. HTTP 404 maps to an explicit missing-caption error rather than a blank
   message;
10. no upload endpoint is emitted by current code;
11. a false exposure flag means no frontend menu item but does not alter the
    underlying endpoint paths.

Any remote coverage must remain behind
`DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER`, serialize mutations through
`REMOTE_TEST_LOCK`, use an existing disposable caption only when cleanup is
already known, and otherwise limit live coverage to listing plus a single
non-mutating missing-ID delete check.
