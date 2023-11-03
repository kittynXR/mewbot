use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct World {
    pub name: String,
    pub description: String,
    #[serde(rename = "authorName")]
    pub author_name: String,
    pub capacity: i32,
    pub id: String,
    #[serde(rename = "releaseStatus")]
    pub release_status: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    #[serde(rename = "type")]
    message_type: String,
    content: Option<String>,
}

pub fn extract_user_location_info(json_message: &str) -> Result<Option<World>, serde_json::Error> {
    //println!("Received JSON: {}", json_message);

    let message: Result<Message, serde_json::Error> = serde_json::from_str(json_message);

    match message {
        Ok(msg) => {
            //println!("Deserialized message: {:?}", msg);
            if msg.message_type == "user-location" {
                if let Some(content) = msg.content {
                    match serde_json::from_str::<Value>(&content) {
                        Ok(content_val) => {
                            if let Some(world_val) = content_val.get("world") {
                                match serde_json::from_value::<World>(world_val.clone()) {
                                    Ok(world) => {
                                        println!("Extracted world: {:?}", world);
                                        Ok(Some(world))
                                    }
                                    Err(err) => {
                                        println!("Failed to deserialize world: {}", err);
                                        Err(err)
                                    }
                                }
                            } else {
                                println!("'world' key not found in content.");
                                Ok(None)
                            }
                        }
                        Err(err) => {
                            println!("Failed to parse content as JSON: {}", err);
                            Err(err)
                        }
                    }
                } else {
                    println!("No content in user-location message.");
                    Ok(None)
                }
            } else {
                println!("Message is not of type 'user-location'.");
                Ok(None)
            }
        },
        Err(err) => {
            println!("Failed to deserialize JSON: {}", err);
            Err(err)
        },
    }
}
