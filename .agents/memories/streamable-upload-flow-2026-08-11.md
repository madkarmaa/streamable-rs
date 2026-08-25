# Streamable web upload flow

Implementation status: **Implemented in `streamable-rs`.**

The wire contract was verified against the live browser flow on 2026-08-11.
Streamable can drift, so revalidate it when upload integration behavior matters.

## Security invariant

Never store live upload credentials, session or transcoder tokens, policies,
signatures, cookies, authorization values, account details, generated
shortcodes, or signed CDN query strings in tracked fixtures or memories. Use
descriptive placeholders when documenting their wire positions.

## Observed wire contract

The browser flow is structurally:

1. `GET /api/v1/uploads/shortcode?size=<bytes>&version=unknown` obtains a
   shortcode, temporary S3 credentials, upload fields, and transcoder options.
2. `POST /api/v1/videos/<shortcode>/initialize` sends `original_size`,
   `original_name`, `upload_source="web"`, and the file-stem `title`.
3. A SigV4-signed `PUT` streams the raw file to
   `https://<bucket>.s3.amazonaws.com/<key>`.
4. `POST /api/v1/transcode/<shortcode>` sends `upload_source="web"` plus the
   returned `key`, `token`, `shortcode`, and file `size`.
5. The browser fetches the processed video state and thumbnail.

The shortcode response uses `transcoder_options.key`, not `url`. Its upload
fields omit `acl`. The S3 request likewise neither transmits nor signs
`x-amz-acl`; its signed headers are `host`, `x-amz-content-sha256`, `x-amz-date`,
`x-amz-security-token`, and `x-amz-user-agent`. The observed payload mode is
`UNSIGNED-PAYLOAD`, and the credential scope uses `us-east-1`.

The browser also emits log, completion, and progress telemetry. Those requests
are informational and are intentionally absent from the Rust upload flow.

## Rust upload lifecycle

`StreamableClient::upload_video` implements the flow for authenticated and
unauthenticated clients and returns the `Video` from the transcode response. It
delegates to `begin_video_upload().complete()`.

`StreamableClient::begin_video_upload` validates the file, allocates a shortcode,
and returns `VideoUpload` without calling `/initialize`. `VideoUpload` is
`#[must_use]` because dropping it abandons the allocated remote resource.
`VideoUpload::complete` initializes the upload, streams the file through the
configured transport, and requests transcoding.

Applications cancel an in-flight completion future through their runtime and
retain a `VideoUploadHandle` for explicit cleanup. `VideoUpload::cancel` consumes
an allocated upload, while `VideoUpload::handle` keeps cleanup available when
another task owns the completion future.

After shortcode allocation, library-detected failures attempt one bodyless
`POST /api/v1/videos/<shortcode>/cancel`. If upload and rollback both fail,
`StreamableError::UploadRollback` preserves both errors. Dropping a future cannot
perform async cleanup, so runtime-level cancellation requires the retained
handle.

## Verification

Deterministic tests assert the effective paths, query, request order, JSON wire
names, streamed file body, signed headers, absence of `x-amz-acl`, bodyless
cancellation, rollback behavior, and shortcode HTTP 429 mapping. Feature-gated
remote tests cover one retained upload and one cancellation where cancellation
itself is the behavior under test. Default tests remain offline.
