use nestrs::prelude::*;

use crate::app_controller::AppController;
use crate::app_service::AppService;
use crate::jobs_processor::{JobsProcessor, JOBS_QUEUE};

#[module(
    imports = [QueuesModule::register(&[QueueConfig::new(JOBS_QUEUE)])],
    controllers = [AppController],
    providers = [AppService, JobsProcessor],
)]
pub struct AppModule;
