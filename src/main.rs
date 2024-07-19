use mewbot::{config::Config, init, run};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::new()?;
    let config = Arc::new(RwLock::new(config));
    let clients = init(Arc::clone(&config)).await?;

    run(clients, config).await?;

    Ok(())
}