# `streamable`

Unofficial async Rust client for Streamable's undocumented API.

## API coverage

- Videos: upload, fetch, delete, thumbnails, privacy, analytics, and live views.
- Collections: create, list, fetch, update, and delete.
- Accounts: register, sign in, refresh, change password, and set defaults.
- Labels: create, rename, delete, and assign to videos.

Video and collection operations work without signing in. Account and label operations require a
signed-in client.

Returned videos, labels, collections, and uploads retain their client session and expose their own
remote operations.

## Usage

```no_run
use streamable::{models::VideoPrivacySettingsUpdate, Result, StreamableClient};

# async fn run() -> Result<()> {
let client = StreamableClient::new()?;
let mut video = client.upload_video("video.mp4", None).await?;
video.update_privacy(&VideoPrivacySettingsUpdate {
    allow_download: Some(false),
    ..Default::default()
}).await?;
video.set_thumbnail_frame(1.5).await?;
# Ok(()) }
```

Sign in for account settings and labels:

```no_run
use streamable::{Result, StreamableClient};

# async fn run() -> Result<()> {
let client = StreamableClient::new()?
    .login("me@example.com".into(), "password".into()).await?;
let label = client.create_label("reviewed").await?;
let mut video = client.get_video("abc123").await?;
video.set_labels(&[label.id]).await?;
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
`transport::HttpTransport`. Raw file uploads use `transport::Body::File`; custom thumbnail uploads
use `transport::Body::MultipartFile`.

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
