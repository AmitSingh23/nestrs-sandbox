use std::sync::Arc;

use nestrs::prelude::*;

use crate::fight_news::processor::FIGHT_NEWS_QUEUE;

/// Queues a fight news scrape at startup and then every 6 hours.
#[injectable]
pub struct FightNewsScheduler {
    queues: Arc<QueuesService>,
}

#[schedule_routes]
impl FightNewsScheduler {
    /// nestrs runs interval tasks once at startup as well.
    #[interval(21_600_000)]
    async fn every_six_hours(&self) {
        let queue = self.queues.expect_queue(FIGHT_NEWS_QUEUE);
        if let Err(err) = queue.add("scrape", &serde_json::json!({})).await {
            tracing::error!(%err, "failed to queue fight news scrape");
        }
    }
}
