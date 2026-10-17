use std::sync::Arc;

use nestrs::prelude::*;
use nestrs::sea_orm::DbConn;

use crate::fight_news::scraper;

pub const FIGHT_NEWS_QUEUE: &str = "FIGHT_NEWS";

/// Scrapes UFC news outlets for upcoming and rumored fights.
#[injectable]
#[queue_processor("FIGHT_NEWS")]
pub struct FightNewsProcessor {
    db: Arc<DbConn>,
}

#[async_trait]
impl QueueHandler for FightNewsProcessor {
    async fn process(&self, _job: QueueJob) -> Result<(), QueueError> {
        let summary = scraper::scrape_all(&self.db)
            .await
            .map_err(|e| QueueError::new(e.to_string()))?;
        tracing::info!(?summary, "fight news scrape finished");
        Ok(())
    }
}
