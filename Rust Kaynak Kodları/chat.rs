use serde::{Deserialize, Serialize};

/// Chat message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub from: String,
    pub content: String,
    pub timestamp: u64,
}

impl ChatMessage {
    pub fn new(from: String, content: String) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            from,
            content,
            timestamp,
        }
    }
}
