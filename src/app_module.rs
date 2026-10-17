use nestrs::prelude::*;
use nestrs::sea_orm::SeaOrmModule;

use crate::app_controller::AppController;
use crate::app_service::AppService;
use crate::database;
use crate::fight_news::processor::{FightNewsProcessor, FIGHT_NEWS_QUEUE};
use crate::fight_news::scheduler::FightNewsScheduler;
use crate::jobs_processor::{JobsProcessor, JOBS_QUEUE};

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
