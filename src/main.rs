use std::env;
use std::sync::{Arc};
use tokio::sync::Mutex as AsyncMutex;

use twitch_irc::TwitchIRCClient;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::{ClientConfig, SecureTCPTransport};
use twitch_irc::message::ServerMessage;

use futures_util::{StreamExt};

use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;

use tokio_tungstenite::{connect_async_tls_with_config, Connector};
use tokio_tungstenite::tungstenite::http::{Request, Uri};
use tokio_tungstenite::tungstenite::http::{header, HeaderMap, HeaderValue};

use std::time::Duration;
use tokio::time::sleep;
use mewbot::extract_user_location_info;
use mewbot::World;


#[tokio::main]
async fn main() {
    let wrld = Arc::new(AsyncMutex::new(None::<World>));

    let ttvtoken = match env::var("CAT_TTV_TOKEN") {
        Ok(token) => {
            //println!("Using token {}", token);
            token
        },
        Err(e) => {
            println!("Couldn't read login token for twitch: ({})", e);
            String::new()
        },
    };

    if !ttvtoken.is_empty() {
        // Do something with ttvtoken
        println!("Token is available for use!");
    } else {
        println!("No token was found.");
    }

    let vrc_authcookie = match env::var("VRC_AUTHCOOKIE") {
        Ok(token) => {
            token
        },
        Err(e) => {
            println!("Couldn't read authcookie for VRC: ({})", e);
            String::new()
        },
    };

    if !vrc_authcookie.is_empty() {
        println!("vrc token ready!");
    } else {
        println!("no vrc token found");
    }

    let ttv_credential_pair = StaticLoginCredentials::new("catcatmewmew".to_string(), Some(ttvtoken));
    let config = ClientConfig::new_simple(ttv_credential_pair);
    // Create a Twitch IRC client with your credentials
    let (mut incoming_messages, client) =
        TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

    let chatclient = client.clone();

    let twitch_wrld = Arc::clone(&wrld);
    let twitch_handle = tokio::spawn(async move {
        client.join("kittyn".to_owned()).unwrap();
        while let Some(message) = incoming_messages.recv().await {
            match message {
                ServerMessage::Privmsg(msg) => {
                    println!("(#{}) {}: {}", msg.channel_login, msg.sender.name, msg.message_text);
                    if msg.message_text == "!world" {
                        let twitch_guard = twitch_wrld.lock().await;
                        match &*twitch_guard {
                            Some(world) => {
                                chatclient.say("kittyn".to_owned(), format!("World Name: {}", world.name).to_owned()).await.unwrap();
                                sleep(Duration::from_millis(1250)).await;
                                match chatclient.say("kittyn".to_owned(), format!("Author: {}   Capacity: {}", world.author_name, world.capacity).to_owned()).await {
                                    Ok(_) => {},
                                    Err(e) => eprintln!("Error sending message: {}", e),
                                }
                                sleep(Duration::from_millis(1250)).await;
                                chatclient.say("kittyn".to_owned(), format!("Description: {}", world.description).to_owned()).await.unwrap();
                                sleep(Duration::from_millis(1250)).await;
                                if world.release_status != "private" {
                                    chatclient.say("kittyn".to_owned(), format!("{}: https://vrchat.com/home/world/{}", world.release_status, world.id).to_owned()).await.unwrap();
                                } else {
                                    chatclient.say("kittyn".to_owned(), format!("{}: No link available", world.release_status).to_owned()).await.unwrap();
                                }
                            }
                            None => {
                                chatclient.say("kittyn".to_owned(), "No world information available yet.".to_owned()).await.unwrap();
                            }
                        }                    }
                },
                ServerMessage::Whisper(msg) => {
                    println!("(w) {}: {}", msg.sender.name, msg.message_text);
                },
                _ => {}
            }
        }
    });

    // Create a WebSocket request
    let url: Uri = format!("wss://pipeline.vrchat.cloud/?authToken={}", vrc_authcookie).parse().unwrap();

    let mut headers = HeaderMap::new();
    headers.insert(header::USER_AGENT, HeaderValue::from_static("osCatNet kittynvrc@gmail.com"));

    let request = Request::builder()
        .uri(&url)
        .header("Host", "pipeline.vrchat.cloud")
        .header("Origin", "kittyn.cat")
        .header("Connection", "Upgrade")
        .header("Upgrade", "WebSocket")
        .header("User-Agent", "osCatNet kittynvrc@gmail.com")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
        .body(())
        .unwrap();

    let tls_connector = native_tls::TlsConnector::builder().build();
    let connector = tls_connector.expect("connector not built");
    let conx = Connector::NativeTls(connector.clone());

    let (ws_stream, _) = connect_async_tls_with_config(
        request,
        None,
        false,
        Option::from(conx)
    )
        .await
        .expect("Failed to connect");
    println!("Websocket connection established");

    let (_write, mut read) = ws_stream.split();
    let websocket_wrld = Arc::clone(&wrld);

    let ws_handle = tokio::spawn(async move {
        while let Some(result) = read.next().await {
            match result {
                Ok(TungsteniteMessage::Text(msg)) => {
                    match extract_user_location_info(&msg) {
                        Ok(Some(world)) => {
                            let mut wrld_guard = websocket_wrld.lock().await;
                            *wrld_guard = Some(world);
                            println!("Received a user-location message with world: {:?}", wrld);
                        }
                        Ok(None) => {
                            println!("Received a message, but it's not of type 'user-location' or doesn't contain a 'world'.");
                        }
                        Err(err) => {
                            println!("Failed to extract user location info: {}", err);
                        }
                    }
                }
                Ok(_) => {
                    println!("Received a non-text message, skipping.");
                }
                Err(err) => {
                    println!("WebSocket error: {}", err);
                    break;
                }
            }
        }
    });
    tokio::try_join!(twitch_handle, ws_handle).unwrap();
}