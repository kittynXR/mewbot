use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::twitch::models::RedeemInfo;

pub struct RedeemRegistry {
    redeems: RwLock<HashMap<String, RedeemInfo>>,
}

impl RedeemRegistry {
    pub fn new() -> Self {
        Self {
            redeems: RwLock::new(HashMap::new()),
        }
    }

    pub async fn add_or_update(&self, title: String, info: RedeemInfo) {
        let mut redeems = self.redeems.write().await;
        redeems.insert(title, info);
    }

    pub async fn get(&self, title: &str) -> Option<RedeemInfo> {
        let redeems = self.redeems.read().await;
        redeems.get(title).cloned()
    }

    pub async fn get_all(&self) -> Vec<RedeemInfo> {
        let redeems = self.redeems.read().await;
        redeems.values().cloned().collect()
    }

    pub async fn remove(&self, title: &str) {
        let mut redeems = self.redeems.write().await;
        redeems.remove(title);
    }
}