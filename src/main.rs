use mewbot::{Config, init, run};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::new()?;
    let (twitch_client, vrchat_client) = init(config).await?;
    run(twitch_client, vrchat_client).await
}