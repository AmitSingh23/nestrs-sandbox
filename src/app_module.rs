use nestrs::prelude::*;
use nestrs::sea_orm::SeaOrmModule;

use crate::app_controller::AppController;
use crate::app_service::AppService;
use crate::database;
use crate::fight_news::processor::{FIGHT_NEWS_QUEUE, FightNewsProcessor};
use crate::fight_news::scheduler::FightNewsScheduler;
use crate::jobs_processor::{JOBS_QUEUE, JobsProcessor};

#[module(
    imports = [
        SeaOrmModule::from_connection(database::connection()),
        QueuesModule::register(&[
            QueueConfig::new(JOBS_QUEUE),
            QueueConfig::new(FIGHT_NEWS_QUEUE),
        ]),
        ScheduleModule::for_root(),
    ],
    controllers = [AppController],
    providers = [AppService, JobsProcessor, FightNewsProcessor, FightNewsScheduler],
)]
pub struct AppModule;
