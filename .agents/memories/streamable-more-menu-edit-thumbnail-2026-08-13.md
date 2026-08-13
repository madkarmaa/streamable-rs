# Streamable dashboard More menu: Edit thumbnail

Captured 2026-08-13 from current Streamable web UI, production source maps,
and Chrome DevTools network inspection. This file records only the **Edit
thumbnail** feature and its two save branches. Temporary video IDs, session
cookies, signed CDN parameters, account data, and telemetry identifiers are
intentionally omitted.

## Entry point

For a ready, non-expired video card, **More > Edit thumbnail** performs a
client-side navigation to:

```text
/thumbnail/<shortcode>
```

The page has a back control and a single `Save Changes` action. The mutation is
deferred until that action; moving the slider, editing the time input, choosing
an image, or switching back to a video frame does not call a thumbnail API.

## Initial data and preview flow

The thumbnail page first looks for the video in the dashboard cache. If the
video or its best playable source is absent, it fetches:

```text
GET https://api-f.streamable.com/api/v1/videos/<shortcode>
```

When a truthy route version is supplied, the request becomes:

```text
GET /api/v1/videos/<shortcode>?version=<version>
```

The page uses the best source URL from the video response. Chrome then requests
the signed CDN video URL, commonly with one or more HTTP range requests. The
live inspection returned HTTP 206 while the page sought to the preview frame.
Signed CDN query parameters are ephemeral and must never be persisted.

## Branch 1: select a video frame

The frame selector contains:

- a slider with 0.01-second steps;
- a text input that displays `minutes:seconds`, or
  `hours:minutes:seconds` for long videos;
- a live video preview that seeks as the value changes.

The slider maximum is `video.duration * 0.95`, not the full duration. In the
observed 52-second video this produced a maximum of 49.4 seconds.

The time parser accepts colon-separated sections and computes seconds from the
right. Blank sections are treated as zero, and values use `parseFloat`. The
display formatter floors to one decimal place rather than rounding.

### Default-offset rules

The page parses the persisted `thumbnail_offset` with `parseInt`:

- `thumbnail_offset == -1` means a custom uploaded image;
- a custom image or persisted offset `0` defaults the frame selector to
  `duration / 2`;
- any other persisted offset becomes the default frame.

Because save code uses `state.thumbOffset || defaultOffset`, an explicitly
selected numeric zero is treated as false and is replaced with the default
offset. This is current UI behavior and should not be silently copied into a
future Rust API unless strict UI parity requires it.

### Frame-save request

`Save Changes` sends:

```http
PATCH https://api-f.streamable.com/api/v1/screenshots/<shortcode>
Content-Type: application/json
Cookie: <session; redacted>

{"thumbOffset":26}
```

Important wire details:

- JSON field name is camelCase `thumbOffset`;
- authentication is the normal session cookie;
- the request body contains only the offset;
- success is HTTP 200 with the complete updated video object.

The live response changed these meaningful fields:

```json
{
  "thumbnail_offset": "26",
  "dynamic_thumbnail_url": "//cdn-cf-east.streamable.com/image/<shortcode>-screenshot<generated>.jpg"
}
```

The response uses string `thumbnail_offset` even though the request sends a
number.

## Branch 2: upload a custom image

Desktop UI exposes `Upload Image` through a hidden file input:

```html
<input id="upload-input" type="file" accept="image/*">
```

The upload button is hidden on mobile. Selecting a file replaces the video
preview with a local object-URL image preview. A custom image also exposes
`Use video frame`, which clears the pending upload and restores the frame
selector without a network request.

### Custom-image request

`Save Changes` sends a multipart request:

```http
POST https://api-f.streamable.com/api/v1/screenshots/<shortcode>/upload
Content-Type: multipart/form-data; boundary=<browser-generated>
Cookie: <session; redacted>

form field: screenshot=<image file>
```

Important wire details:

- method is POST;
- multipart field name is exactly `screenshot`;
- browser generates the multipart boundary;
- the request preserves the selected file name and media type;
- success is HTTP 200 with the complete updated video object.

The live request uploaded a small PNG. The service returned a generated JPEG
thumbnail URL and marked the custom-image sentinel:

```json
{
  "thumbnail_offset": "-1",
  "dynamic_thumbnail_url": "//cdn-cf-east.streamable.com/image/upload-<shortcode>-<generated>.jpg"
}
```

Thus `-1` is the observable custom-thumbnail marker. The service may normalize
the uploaded format to JPEG; clients must use the returned URL rather than
constructing one from the original file name.

## Completion and error behavior

Both branches:

1. disable `Save Changes` while the request is active;
2. parse the response as JSON;
3. dispatch the returned video object into dashboard state;
4. navigate back in browser history on success;
5. re-enable the action and show an alert containing the thrown message on
   failure.

Current request helpers expect a JSON body even for non-success responses. They
log the server `message` and throw it when the status is not successful.

## Live restoration check

After exercising custom-image upload, the disposable test video was restored by
sending the frame PATCH again. Final observed state was:

```json
{
  "status": 2,
  "thumbnail_offset": "26",
  "dynamic_thumbnail_url": "//cdn-cf-east.streamable.com/image/<shortcode>-screenshot<generated>.jpg"
}
```

This proves the two branches are reversible at the API level for this guest
video: a later frame selection replaces the custom-image sentinel.

## Future Rust implementation guidance

Model the branches as separate typed operations:

- `set_video_thumbnail_frame(shortcode, seconds)` using JSON PATCH;
- `upload_video_thumbnail(shortcode, image)` using multipart POST.

Do not expose generated CDN naming rules as stable API. Preserve the response's
string offset unless a validated conversion layer explicitly accepts both JSON
strings and numbers. Validate finite, non-negative seconds in the Rust API even
though the current browser time parser is permissive, but keep server error
responses available for compatibility diagnostics.

## Deterministic test targets

Local mock coverage should verify:

1. frame selection uses PATCH on the full
   `/api/v1/screenshots/<shortcode>` path;
2. its JSON is exactly `{ "thumbOffset": <number> }`;
3. custom upload uses POST on
   `/api/v1/screenshots/<shortcode>/upload`;
4. multipart contains one `screenshot` file part with original bytes and media
   type;
5. both requests carry the authenticated session cookie;
6. HTTP 200 deserializes the returned video, including offsets `"-1"` and a
   normal numeric string;
7. non-success JSON `message` values map to an explicit thumbnail error;
8. signed CDN URLs are treated as opaque response data.

Any remote coverage must remain behind
`DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER`, use a disposable video, and
restore the original thumbnail branch after verification.
