//! Mock implementations for feed-related repository traits.

use async_trait::async_trait;
use chrono::Utc;
use domain::entity_feed::{EntityFeed, EntityFeedError, EntityFeedRepository, EntityType};
use domain::feed::{Feed, FeedError, FeedRepository, FeedType};
use domain::feed_element::{FeedElement, FeedElementError, FeedElementRepository, FeedElementType};
use domain::feed_item::{AuthorType, FeedItem, FeedItemError, FeedItemRepository};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct MockFeedRepo {
    pub feeds: Mutex<Vec<Feed>>,
}

#[async_trait]
impl FeedRepository for MockFeedRepo {
    async fn create(
        &self,
        slug: &str,
        display_name: &str,
        description: Option<&str>,
        feed_type: FeedType,
    ) -> Result<Feed, FeedError> {
        let feed = Feed {
            id: Uuid::new_v4(),
            slug: slug.into(),
            display_name: display_name.into(),
            description: description.map(String::from),
            feed_type,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        };
        self.feeds.lock().await.push(feed.clone());
        Ok(feed)
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Feed>, FeedError> {
        Ok(self.feeds.lock().await.iter().find(|f| f.id == id).cloned())
    }
    async fn update(
        &self,
        _id: Uuid,
        _display_name: &str,
        _description: Option<&str>,
    ) -> Result<Feed, FeedError> {
        unimplemented!()
    }
    async fn soft_delete(&self, _id: Uuid) -> Result<(), FeedError> {
        unimplemented!()
    }
    async fn list_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Feed>, FeedError> {
        let feeds = self.feeds.lock().await;
        let result = feeds
            .iter()
            .filter(|f| ids.contains(&f.id))
            .cloned()
            .collect();
        Ok(result)
    }
}

#[derive(Default)]
pub struct MockEntityFeedRepo {
    pub entity_feeds: Mutex<Vec<EntityFeed>>,
}

#[async_trait]
impl EntityFeedRepository for MockEntityFeedRepo {
    async fn attach(
        &self,
        feed_id: Uuid,
        entity_type: EntityType,
        entity_id: Uuid,
    ) -> Result<EntityFeed, EntityFeedError> {
        let ef = EntityFeed {
            feed_id,
            entity_type,
            entity_id,
        };
        self.entity_feeds.lock().await.push(ef.clone());
        Ok(ef)
    }
    async fn find_by_feed_id(
        &self,
        feed_id: Uuid,
    ) -> Result<Option<EntityFeed>, EntityFeedError> {
        Ok(self
            .entity_feeds
            .lock()
            .await
            .iter()
            .find(|ef| ef.feed_id == feed_id)
            .cloned())
    }
    async fn list_by_entity(
        &self,
        entity_type: EntityType,
        entity_id: Uuid,
    ) -> Result<Vec<EntityFeed>, EntityFeedError> {
        let efs = self.entity_feeds.lock().await;
        let result = efs
            .iter()
            .filter(|ef| ef.entity_type == entity_type && ef.entity_id == entity_id)
            .cloned()
            .collect();
        Ok(result)
    }
    async fn detach(&self, _feed_id: Uuid) -> Result<(), EntityFeedError> {
        unimplemented!()
    }
}

#[derive(Default)]
pub struct MockFeedItemRepo {
    pub items: Mutex<Vec<FeedItem>>,
}

#[async_trait]
impl FeedItemRepository for MockFeedItemRepo {
    async fn create(
        &self,
        feed_id: Uuid,
        author_type: AuthorType,
        author_id: Uuid,
    ) -> Result<FeedItem, FeedItemError> {
        let item = FeedItem {
            id: Uuid::new_v4(),
            feed_id,
            author_type,
            author_id,
            created_at: Utc::now(),
        };
        self.items.lock().await.push(item.clone());
        Ok(item)
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<FeedItem>, FeedItemError> {
        Ok(self.items.lock().await.iter().find(|i| i.id == id).cloned())
    }
    async fn list_by_feed(
        &self,
        feed_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FeedItem>, FeedItemError> {
        let items = self.items.lock().await;
        let result = items
            .iter()
            .filter(|i| i.feed_id == feed_id)
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(result)
    }
    async fn delete(&self, id: Uuid) -> Result<(), FeedItemError> {
        let mut items = self.items.lock().await;
        let len_before = items.len();
        items.retain(|i| i.id != id);
        if items.len() == len_before {
            Err(FeedItemError::NotFound)
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
pub struct MockFeedElementRepo {
    pub elements: Mutex<Vec<FeedElement>>,
}

#[async_trait]
impl FeedElementRepository for MockFeedElementRepo {
    async fn create(
        &self,
        feed_item_id: Uuid,
        element_type: FeedElementType,
        content_json: &str,
        position: i32,
    ) -> Result<FeedElement, FeedElementError> {
        let el = FeedElement {
            id: Uuid::new_v4(),
            feed_item_id,
            element_type,
            content_json: content_json.into(),
            position,
        };
        self.elements.lock().await.push(el.clone());
        Ok(el)
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<FeedElement>, FeedElementError> {
        Ok(None)
    }
    async fn list_by_feed_item(
        &self,
        feed_item_id: Uuid,
    ) -> Result<Vec<FeedElement>, FeedElementError> {
        let els = self.elements.lock().await;
        let result = els
            .iter()
            .filter(|e| e.feed_item_id == feed_item_id)
            .cloned()
            .collect();
        Ok(result)
    }
    async fn update_content(
        &self,
        _id: Uuid,
        _content_json: &str,
    ) -> Result<FeedElement, FeedElementError> {
        unimplemented!()
    }
    async fn delete(&self, _id: Uuid) -> Result<(), FeedElementError> {
        unimplemented!()
    }
}
