# Core debug tracing

The core library depends directly on `tracing`.

`StreamableClient::execute` identifies API operations by request model and logs
success or a stable internal error kind. `send_request` records method, endpoint,
body kind/length, cookie presence/counts, status, and response length. Error
events use stable internal variant kinds.

Test-only `ctor` and `tracing-subscriber` dev-dependencies install a test-writer
subscriber under `cfg(test)` when `STREAMABLE_TEST_TRACING` is present. Normal
production dependency resolution contains neither crate. `cargo test -p
streamable-rs` stays quiet. With `STREAMABLE_TEST_TRACING` present and
`-- --no-capture`, the same full offline unit suite emits debug events.
