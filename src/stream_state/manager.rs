use tokio::sync::{broadcast, RwLock};
use std::sync::Arc;
use log::{info, warn};
use serde::{Serialize, Deserialize};
use crate::stream_state::errors::StateTransitionError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamState {
    Offline,
    GoingLive,
    Live(String), // String represents the current game
    GoingOffline,
}

pub struct StreamStateMachine {
    current_state: RwLock<StreamState>,
    transition_channel: broadcast::Sender<StreamState>,
}

impl StreamStateMachine {
    pub fn new() -> Arc<Self> {
        let (transition_channel, _) = broadcast::channel(100);
        Arc::new(Self {
            current_state: RwLock::new(StreamState::Offline),
            transition_channel,
        })
    }

    pub async fn transition(&self, new_state: StreamState) -> Result<(), StateTransitionError> {
        let mut state = self.current_state.write().await;
        let current_state = state.clone();
        info!("Attempting state transition: {:?} -> {:?}", current_state, new_state);

        let transition_allowed = match (&current_state, &new_state) {
            (StreamState::Offline, StreamState::GoingLive) => true,
            (StreamState::GoingLive, StreamState::Live(_)) => true,
            (StreamState::Live(_), StreamState::GoingOffline) => true,
            (StreamState::GoingOffline, StreamState::Offline) => true,
            (StreamState::Live(current_game), StreamState::Live(new_game)) if current_game != new_game => true,
            _ => false,
        };

        if transition_allowed {
            *state = new_state.clone();
            drop(state); // Release the write lock before sending the broadcast

            if let Err(e) = self.transition_channel.send(new_state.clone()) {
                warn!("Failed to broadcast state transition: {:?}", e);
                return Err(StateTransitionError::BroadcastError);
            }
            info!("State transition successful: {:?} -> {:?}", current_state, new_state);
            Ok(())
        } else {
            warn!("Invalid state transition attempted: {:?} -> {:?}", current_state, new_state);
            Err(StateTransitionError::InvalidTransition)
        }
    }

    pub async fn get_current_state(&self) -> StreamState {
        self.current_state.read().await.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamState> {
        self.transition_channel.subscribe()
    }

    pub async fn is_stream_live(&self) -> bool {
        matches!(self.get_current_state().await, StreamState::Live(_))
    }

    pub async fn get_current_game(&self) -> Option<String> {
        match self.get_current_state().await {
            StreamState::Live(game) => Some(game),
            _ => None,
        }
    }

    pub async fn set_stream_live(&self, game_name: String) -> Result<(), StateTransitionError> {
        let current_state = self.get_current_state().await;
        info!("Attempting to set stream live with game: {}. Current state: {:?}", game_name, current_state);

        match current_state {
            StreamState::Live(current_game) if current_game == game_name => {
                info!("Stream is already live with the same game. No action taken.");
                Ok(())
            },
            StreamState::Live(_) => {
                info!("Stream is already live. Updating game to: {}", game_name);
                self.transition(StreamState::Live(game_name)).await
            },
            StreamState::GoingLive => {
                info!("Stream is already going live. Transitioning to Live state with game: {}", game_name);
                self.transition(StreamState::Live(game_name)).await
            },
            _ => {
                info!("Transitioning stream to GoingLive, then Live with game: {}", game_name);
                self.transition(StreamState::GoingLive).await?;
                self.transition(StreamState::Live(game_name)).await
            }
        }
    }

    pub async fn set_stream_offline(&self) -> Result<(), StateTransitionError> {
        let current_state = self.get_current_state().await;
        info!("Attempting to set stream offline. Current state: {:?}", current_state);

        match current_state {
            StreamState::Offline => {
                info!("Stream is already offline. No action taken.");
                Ok(())
            },
            StreamState::GoingOffline => {
                info!("Stream is going offline. Transitioning to Offline state.");
                self.transition(StreamState::Offline).await
            },
            _ => {
                info!("Transitioning stream to GoingOffline, then Offline.");
                self.transition(StreamState::GoingOffline).await?;
                self.transition(StreamState::Offline).await
            }
        }
    }

    pub async fn update_game(&self, new_game: String) -> Result<(), StateTransitionError> {
        let current_state = self.get_current_state().await;
        info!("Attempting to update game to: {}. Current state: {:?}", new_game, current_state);

        match current_state {
            StreamState::Live(current_game) if current_game == new_game => {
                info!("Game is already set to {}. No action taken.", new_game);
                Ok(())
            },
            StreamState::Live(_) => {
                info!("Updating game for live stream to: {}", new_game);
                self.transition(StreamState::Live(new_game)).await
            },
            StreamState::Offline | StreamState::GoingOffline => {
                info!("Stream is offline. Storing new game: {} for when stream goes live.", new_game);
                // Here, you might want to store the game name somewhere for when the stream goes live
                // For now, we'll just acknowledge the update without changing the state
                Ok(())
            },
            StreamState::GoingLive => {
                info!("Stream is going live. Updating game to: {}", new_game);
                self.transition(StreamState::Live(new_game)).await
            },
        }
    }
}