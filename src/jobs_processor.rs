use nestrs::prelude::*;

pub const JOBS_QUEUE: &str = "JOBS";

#[derive(Default)]
#[injectable]
#[queue_processor("JOBS")]
pub struct JobsProcessor;

#[async_trait]
impl QueueHandler for JobsProcessor {
    async fn process(&self, _job: QueueJob) -> Result<(), QueueError> {
        Ok(())
    }
}
