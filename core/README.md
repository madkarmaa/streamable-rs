# `streamable`

Unofficial Rust client for Streamable's undocumented API.

The client uses type states: a new client starts unauthenticated, while
[`StreamableClient::login`] and [`StreamableClient::register`] return an
[`AuthenticatedStreamableClient`]. This prevents authenticated-only account operations from being
called before login.

## Basic usage

```no_run
use streamable::{Result, StreamableClient};

#[tokio::main]
async fn main() -> Result<()> {
    let client = StreamableClient::new()?;
    let client = client
        .login("user@example.com".to_owned(), "password".to_owned())
        .await?;

    println!("signed in as {}", client.user().user_name);
    let video = client.upload_video("example.mp4").await?;
    println!("https://streamable.com/{}", video.shortcode);
    Ok(())
}
```
