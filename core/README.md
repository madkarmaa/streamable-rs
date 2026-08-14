# `streamable`

Unofficial Rust client for Streamable's undocumented API.

Video methods work without signing in:

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

Cancel an upload with a shared token:

```no_run
use streamable::{Result, StreamableClient, UploadCancellationToken};

# async fn run() -> Result<()> {
let client = StreamableClient::new()?;
let token = UploadCancellationToken::new();
let cancel = token.clone();
let upload = client.upload_video_with_cancellation("video.mp4", None, token);
cancel.cancel();
let _ = upload.await;
# Ok(()) }
```
