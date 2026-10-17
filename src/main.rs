mod app_controller;
mod app_module;
mod app_service;
mod jobs_processor;

use nestrs::prelude::*;

use app_module::AppModule;

#[tokio::main]
async fn main() {
    NestFactory::create::<AppModule>().listen_graceful(3000).await;
}
