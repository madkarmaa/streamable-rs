# `streamable`

Unofficial async Rust client for Streamable's undocumented API.

## API coverage

- Videos: upload, fetch, delete, privacy, analytics, and live views.
- Collections: create, list, fetch, update, and delete.
- Accounts: register, sign in, refresh, change password, and set defaults.
- Labels: create, rename, delete, and assign to videos.

Video and collection operations work without signing in. Account and label operations require a
signed-in client.

## Usage

```no_run
use streamable::{models::VideoPrivacySettingsUpdate, Result, StreamableClient};

# async fn run() -> Result<()> {
let client = StreamableClient::new()?;
let video = client.upload_video("video.mp4", None).await?;
client.update_video_privacy(&video.shortcode, &VideoPrivacySettingsUpdate {
    allow_download: Some(false),
    ..Default::default()
}).await?;
# Ok(()) }
```

Sign in for account settings and labels:

```no_run
use streamable::{Result, StreamableClient};

# async fn run() -> Result<()> {
let client = StreamableClient::new()?
    .login("me@example.com".into(), "password".into()).await?;
let label = client.create_label("reviewed").await?;
client.set_video_labels("abc123", &[label.id]).await?;
# Ok(()) }
```

Use an upload handle to cancel an in-progress upload and clean up its Streamable allocation:

```no_run
use streamable::{Result, StreamableClient};

# async fn shutdown() {}
# async fn run() -> Result<()> {
let client = StreamableClient::new()?;
let upload = client.begin_video_upload("video.mp4", None).await?;
let handle = upload.handle();
tokio::select! {
    result = upload.complete() => { result?; }
    () = shutdown() => { handle.cancel().await?; }
}
# Ok(()) }
```

## HTTP transport

The default `reqwest` feature provides `StreamableClient::new()` and streams file uploads with
Tokio. Without default features, use `StreamableClient::with_transport` and implement
`transport::HttpTransport`. File uploads use `transport::Body::File`.

## Testing and tracing

Normal tests are offline. Set `STREAMABLE_TEST_TRACING=1` to print structured request lifecycle
metadata; credentials and request bodies are omitted.

```sh
cargo test -p streamable-rs
STREAMABLE_TEST_TRACING=1 cargo test -p streamable-rs -- --no-capture
```

The remote suite is explicit and may mutate Streamable:

```sh
cargo test --workspace --features DANGEROUSLY_SEND_REQUESTS_TO_REMOTE_SERVER
```
