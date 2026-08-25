# Core debug tracing

The core library depends directly on `tracing` and emits debug spans/events at
the shared request pipeline, response decoder, default transport, video-file
inspection, upload lifecycle, S3-signing, and rollback boundaries. The library
does not install a global subscriber; applications retain subscriber and filter
ownership.

`StreamableClient::execute` identifies API operations by request model and logs
success or a stable internal error kind. `send_request` records method, endpoint,
body kind/length, cookie presence/counts, status, and response length. Endpoint
methods do not duplicate instrumentation from these shared seams.

Debug instrumentation records no raw bodies or header, cookie, password,
generated-credential, or AWS credential values. Instrumented functions skip all
arguments and opt in only selected fields; error events use stable variant kinds
where domain error text could include response data or local paths. The local
logging test captures a login lifecycle and asserts request metadata is present
while email/password payload values are absent.

Test-only `ctor` and `tracing-subscriber` dev-dependencies install a test-writer
subscriber under `cfg(test)` when `STREAMABLE_TEST_TRACING` is present. Normal
production dependency resolution contains neither crate. `cargo test -p
streamable-rs` stays quiet. With `STREAMABLE_TEST_TRACING` present and
`-- --no-capture`, the same full offline unit suite emits debug events.
