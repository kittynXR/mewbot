use tokio::sync::{broadcast, RwLock};
use std::sync::Arc;
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
        let transition_allowed = match (&*state, &new_state) {
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

            self.transition_channel.send(new_state)
                .map_err(|_| StateTransitionError::BroadcastError)?;
            Ok(())
        } else {
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
        self.transition(StreamState::GoingLive).await?;
        self.transition(StreamState::Live(game_name)).await
    }

    pub async fn set_stream_offline(&self) -> Result<(), StateTransitionError> {
        self.transition(StreamState::GoingOffline).await?;
        self.transition(StreamState::Offline).await
    }

    pub async fn update_game(&self, new_game: String) -> Result<(), StateTransitionError> {
        if let StreamState::Live(_) = self.get_current_state().await {
            self.transition(StreamState::Live(new_game)).await
        } else {
            Err(StateTransitionError::InvalidTransition)
        }
    }
}

pub fn create_stream_state_machine() -> Arc<StreamStateMachine> {
    StreamStateMachine::new()
}