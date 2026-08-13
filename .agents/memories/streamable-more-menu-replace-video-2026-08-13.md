# Streamable dashboard More menu: Replace video

Implementation status: **Not implemented in `streamable-rs`.**

Captured 2026-08-13 from the authenticated Streamable dashboard, production
source maps, Chrome DevTools network inspection, and comparison with the
sibling Python client. This file records only the **Replace video** feature and
its validation, metadata, signed upload, transcode, polling, preservation, and
failure branches. Cookies, temporary credentials, tokens, signatures, signed
URLs, concrete shortcodes, timestamps, and account data are intentionally
omitted.

## Entry and product behavior

**More > Replace video** opens a local modal; opening it makes no API request.
The modal states:

```text
Upload a new file to replace the original video forever but keep the
customisations, URL, and views associated with its video page and anywhere
it’s embedded.
```

The modal has no separate confirmation step. Selecting a valid file starts the
replacement immediately, marks the card as uploading, clears its local poster
and thumbnail placeholders, and closes the modal before requesting upload
metadata.

The current operation preserves the existing shortcode and public URL. It does
not create or initialize a new video record.

## Local file validation

The hidden dropzone input is:

```html
<input type="file" accept="video/*" multiple="false" />
```

Drag-and-drop can still provide multiple files, so validation runs in this
order:

1. reject more than one file;
2. require the first file's MIME type to contain `video/`;
3. if the account has a size limit, reject a size greater than
   `plan_max_size * ONE_GB_IN_BYTES`;
4. if the account has a duration limit, load the media duration locally and
   reject a longer video.

The exact UI errors are:

```text
Can’t upload. Please select only one video
Can’t upload. Use a video in one of these formats: .mp4, .h264, .mov or .avi
Can’t upload. Your video is too large
Can’t upload. Your video is too long
Can’t upload. File validation failed
There was an error while processing your video file
```

Size and duration failures also trigger the existing plan-upsell reasons
`video-toolarge` and `video-toolong`. File-count and MIME failures do not.

The live inspection intentionally passed a file containing a 404 HTML response
with MIME type `text/html`. The modal produced the file-format error locally
and sent no replace-metadata request. A subsequent valid MP4 proceeded through
the complete flow.

## Step 1: request replacement upload metadata

For a valid file, the first request is:

```http
GET https://api-f.streamable.com/api/v1/uploads/<shortcode>/replace?size=<decimal-byte-count>
Content-Type: application/json
Pragma: no-cache
Cache-Control: no-cache
Cookie: <session; redacted>
```

Important differences from a new upload:

- path contains the existing shortcode and `/replace`;
- query contains only `size`;
- there is no `version=unknown` query;
- there is no subsequent `/videos/<shortcode>/initialize` call.

The live 3,632,116-byte file produced HTTP 200. The response contained:

```text
shortcode: existing shortcode
version: 2
key: upload/replace/<shortcode>-2-<timestamp>
bucket: streamables-upload
accelerated: false
time: server clock reference
credentials: temporary AWS credentials
fields: signed upload fields
url: signed upload URL
options:
  preset: mp4
  shortcode: existing shortcode
  screenshot: true
transcoder_options:
  key: same replacement object key
  token: temporary transcode token
  shortcode: existing shortcode
  size: 3632116
  version: 2
video: current video representation
```

The previous video had `version:1` and `max_version:1`; metadata allocated
replacement version 2. The response's nested current-video snapshot still had
`max_version:1` because the replacement had not yet been uploaded.

If the metadata response lacks `shortcode`, the client fails with
`No shortcode present in metadata` and does not upload bytes.

All credentials, tokens, policies, signatures, object keys with timestamps,
and URLs from this response are ephemeral and must be treated as opaque secret
material.

## Step 2: upload the file to S3

The browser uploads the original file bytes directly to the returned bucket
and key. The successful live request was equivalent to:

```http
PUT https://streamables-upload.s3.amazonaws.com/upload/replace/<shortcode>-2-<timestamp>
Content-Type: application/octet-stream
Content-Length: 3632116
Authorization: <AWS Signature Version 4; redacted>
X-Amz-Date: <ephemeral>
X-Amz-Security-Token: <redacted>
X-Amz-Content-Sha256: <payload hash>

<exact MP4 bytes>
```

The S3 response was HTTP 200.

The upload implementation uses:

- metadata server time to compensate for local clock skew;
- AWS region `us-east-1`;
- accelerated endpoint mode from response metadata;
- up to 15 SDK retries;
- retry delay `retries * 1000` milliseconds;
- managed-upload `queueSize: 3`;
- progress events with percent, speed, and retry count;
- a global client upload queue with concurrency 3.

When three uploads are already running, a new task is marked queued. Recent
speed samples are clamped to non-negative values and averaged over at most 100
samples. UI progress callbacks are throttled after the first 1000 callbacks.

The options passed into the upload task merge response `options` with:

```json
{ "upload_source": "web" }
```

### S3 fallback branch

If the SDK upload fails, the client logs selected networking diagnostics and
falls back to an XHR multipart POST to the metadata `url` unless the task was
aborted. That form contains:

1. all metadata `fields`;
2. the video under multipart field `file`;
3. upload options as form fields only when response `transcoder` is truthy.

Any 2xx or 3xx XHR status is accepted. Other statuses map to
`Cannot connect to server. (<status>)`. A network event with no message maps to
`Network error. Please try again.`

Cancellation aborts the active AWS upload and any fallback XHR. A completed
task ignores later cancel attempts.

## Step 3: start transcoding

After upload success, the card enters `waitingToTranscode`. A global scheduler
checks pending transcodes. If more than two uploads are already in processing
state, it keeps this replacement queued. Otherwise it clears the waiting flag
and merges:

```text
local transcode options
metadata.transcoder_options, when present
otherwise metadata.options
```

Response metadata wins for overlapping fields. It then sends:

```http
POST https://api-f.streamable.com/api/v1/transcode/<shortcode>
Content-Type: application/json
Cookie: <session; redacted>

{
  "preset": "mp4",
  "shortcode": "<shortcode>",
  "screenshot": true,
  "key": "upload/replace/<shortcode>-2-<timestamp>",
  "token": "<redacted>",
  "size": 3632116,
  "version": 2
}
```

The observed response was HTTP 200 with a full video representation. It already
reported `max_version:2`, but the response still exposed the prior active
`version:1` while the version-specific state was being resolved.

HTTP 429 maps to:

```text
You’re uploading too much… please wait a little bit before trying again.
```

Other new-API failures parse the JSON `message`. A transcode error marks the
card as failed and is captured by the frontend error reporter.

An auxiliary telemetry request appeared between the successful S3 PUT and the
transcode request:

```http
POST /api/v1/log

{"message":"Unknown Error","version":"unknown"}
```

It returned HTTP 204. The replacement source does not make this request part of
the required protocol, so a future Rust implementation should not reproduce it
as a functional step.

## Step 4: poll the allocated version

While the local card has processing status, the dashboard polls:

```http
GET https://api-f.streamable.com/api/v1/videos/<shortcode>?version=<max_version>
Content-Type: application/json
Pragma: no-cache
Cache-Control: no-cache
Cookie: <session; redacted>
```

The live version-2 GET returned HTTP 200 with:

```json
{
    "status": 2,
    "percent": 100,
    "version": 2,
    "max_version": 2,
    "duration": 52,
    "original_size": 3632116,
    "available_file_resolutions": ["mp4"]
}
```

Its `files.mp4` entry also had `status:2`, `percent:100`, `version:2`, the
uploaded byte size, 640x360 dimensions, and `reencoded:false`. The content was
already compatible with the requested MP4 preset, so completion was nearly
immediate and only one observed version-specific poll was needed.

The polling path uses `max_version || 0`; it does not poll the unversioned video
while a replacement is processing. A five-second local timeout marks the
dashboard network-error state if a poll does not resolve.

## Preservation observed across replacement

The live replacement used content-equivalent MP4 bytes so behavior could be
preserved while the irreversible version counter advanced. After completion:

- shortcode and public URL were unchanged;
- title was unchanged;
- original display filename remained the pre-existing filename, not the newly
  selected local file name;
- views remained unchanged;
- thumbnail offset remained `"26"`;
- labels and captions remained unchanged;
- privacy fields remained unchanged, including `is_custom:false`;
- only `version` and `max_version` advanced from 1 to 2;
- final media dimensions, duration, and size matched the selected replacement.

This confirms that Replace video is a versioned media-body replacement, not a
general video-metadata overwrite. Fixed card customizations should not be
resent in the transcode payload.

## UI and failure state

As soon as a valid file is accepted, the client optimistically sets:

```text
status: uploading
thumbnail_url: null
poster_url: null
upload_percent: 0
percent: 0
```

After metadata arrives, it also sets both local `version` and `max_version` to
the allocated version. Upload success moves the card to 100% uploaded and
waiting for transcode. Final server polling replaces the optimistic state.

Metadata, S3 setup, or orchestration failure:

1. extracts a user-facing message;
2. shows it in a toast;
3. marks the existing card as error with that message;
4. dispatches a replacement-failed action;
5. logs the message;
6. can open the storage-plan upsell when a target plan is available.

The product copy calls replacement permanent. There is no cleanup endpoint in
this flow that decrements `max_version`, so remote tests should use
content-equivalent data or a disposable video rather than claiming rollback of
the version counter.

## Python parity and future Rust API

The sibling Python implementation has the new-upload flow but no Replace video
operation. The Rust client already has reusable AWS signing/upload machinery
for new uploads, but replacement must not call the normal shortcode or
initialize endpoints.

A future Rust orchestration can be modeled as:

```text
request_video_replacement(shortcode, size)
upload_replacement_bytes(metadata, file)
start_replacement_transcode(metadata)
poll_video_version(shortcode, version)
replace_video(shortcode, file)
```

Preserve response-provided key, version, token, bucket, credentials, clock
time, acceleration choice, `options`, and `transcoder_options` as opaque wire
data. Redact them from logs and errors. Reuse the existing cancellation token
for the S3 stage, and make it explicit that cancellation after the transcode
request cannot restore the prior version.

## Deterministic test targets

Local coverage should verify:

1. invalid MIME, count, size, and duration fail before network access;
2. metadata uses GET on
   `/api/v1/uploads/<shortcode>/replace?size=<bytes>` with no extra query;
3. no shortcode-generation or initialize request occurs;
4. missing metadata shortcode produces a typed error before S3;
5. the exact file bytes go to the returned bucket/key with AWS headers;
6. transcode body merges `options` and response transcode options with the
   correct precedence;
7. transcode includes allocated `version`, `size`, `key`, `token`, `preset`,
   `shortcode`, and `screenshot` wire names;
8. polling uses `/videos/<shortcode>?version=<allocated-version>`;
9. completion accepts `status:2` and preserves unrelated video fields;
10. HTTP 429 and JSON-message failures map to explicit replacement errors;
11. cancellation stops S3 work and makes no later transcode request;
12. temporary credentials, tokens, signatures, policies, and signed URLs are
    absent from formatted diagnostics.

Any remote coverage must remain behind
`DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER`, serialize through
`REMOTE_TEST_LOCK`, use a disposable video or content-equivalent replacement,
make one replacement, and prove the final allocated version without claiming
that the version increment was rolled back.
