//! FeedElement — a single content block within a feed item.
//!
//! Multiple elements compose a post (e.g., text + image + file attachment).
//!
//! ARCHITECTURE DECISIONS:
//!   `content_json` is stored as a String (not serde_json::Value) to keep the
//!   domain crate free of I/O dependencies. Deserialization and validation
//!   happen at the application/API layer before persistence.

use uuid::Uuid;

/// The type of content this element represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedElementType {
    Text,
    Image,
    File,
    Event,
    Embed,
}

impl FeedElementType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedElementType::Text => "text",
            FeedElementType::Image => "image",
            FeedElementType::File => "file",
            FeedElementType::Event => "event",
            FeedElementType::Embed => "embed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "text" => Some(FeedElementType::Text),
            "image" => Some(FeedElementType::Image),
            "file" => Some(FeedElementType::File),
            "event" => Some(FeedElementType::Event),
            "embed" => Some(FeedElementType::Embed),
            _ => None,
        }
    }
}

impl From<FeedElementType> for &'static str {
    fn from(fet: FeedElementType) -> Self {
        fet.as_str()
    }
}

impl TryFrom<&str> for FeedElementType {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        FeedElementType::from_str(s).ok_or_else(|| format!("Unknown element type: {s}"))
    }
}

#[derive(Debug, Clone)]
pub struct FeedElement {
    pub id: Uuid,
    pub feed_item_id: Uuid,
    pub element_type: FeedElementType,
    pub content_json: String,
    pub position: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum FeedElementError {
    #[error("Feed element not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
}

#[async_trait::async_trait]
pub trait FeedElementRepository: Send + Sync {
    async fn create(
        &self,
        feed_item_id: Uuid,
        element_type: FeedElementType,
        content_json: &str,
        position: i32,
    ) -> Result<FeedElement, FeedElementError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<FeedElement>, FeedElementError>;

    async fn list_by_feed_item(
        &self,
        feed_item_id: Uuid,
    ) -> Result<Vec<FeedElement>, FeedElementError>;

    async fn update_content(
        &self,
        id: Uuid,
        content_json: &str,
    ) -> Result<FeedElement, FeedElementError>;

    async fn delete(&self, id: Uuid) -> Result<(), FeedElementError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_element_type_round_trip() {
        let variants = [
            (FeedElementType::Text, "text"),
            (FeedElementType::Image, "image"),
            (FeedElementType::File, "file"),
            (FeedElementType::Event, "event"),
            (FeedElementType::Embed, "embed"),
        ];
        for (variant, s) in variants {
            assert_eq!(variant.as_str(), s);
            assert_eq!(FeedElementType::from_str(s), Some(variant));
        }
    }

    #[test]
    fn feed_element_type_from_str_returns_none_for_unknown() {
        assert_eq!(FeedElementType::from_str("video"), None);
        assert_eq!(FeedElementType::from_str(""), None);
    }
}
