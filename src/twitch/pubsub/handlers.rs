use serde_json::Value;

pub async fn handle_stream_update(data: &Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(message) = data.get("message") {
        let parsed: Value = serde_json::from_str(message.as_str().unwrap_or("{}"))?;

        let title = parsed["title"].as_str().unwrap_or("N/A");
        let game = parsed["category"]["name"].as_str().unwrap_or("N/A");

        println!("Stream Updated: Title: '{}', Game: '{}'", title, game);

        // Here you can add logic to send this information to your Twitch chat,
        // update a database, or perform any other actions you want when the stream is updated.
    }

    Ok(())
}