use std::path::Path;
use streamable::{Result, StreamableClient};

#[tokio::main]
async fn main() -> Result<()> {
    let client = StreamableClient::new()?;

    let (client, _email, _password) = client.register(None, None, None).await?;
    println!("signed in as {}", client.user().user_name);

    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../media/videos/mp4-99mb-sample.mp4");
    let video = client.upload_video(file, Some("My Video".into())).await?;
    println!("https://streamable.com/{}", video.shortcode);

    Ok(())
}
