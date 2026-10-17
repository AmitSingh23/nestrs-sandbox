use nestrs::prelude::*;
use nestrs::sea_orm::SeaOrmModule;

use crate::app_controller::AppController;
use crate::app_service::AppService;
use crate::database;
use crate::jobs_processor::{JobsProcessor, JOBS_QUEUE};

#[module(
    imports = [
        SeaOrmModule::from_connection(database::connection()),
        QueuesModule::register(&[QueueConfig::new(JOBS_QUEUE)]),
    ],
    controllers = [AppController],
    providers = [AppService, JobsProcessor],
)]
pub struct AppModule;
