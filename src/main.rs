mod app_controller;
mod app_module;
mod app_service;
mod database;
mod entities;
mod fight_news;
mod jobs_processor;
mod migration;

use nestrs::prelude::*;

use app_module::AppModule;

#[tokio::main]
async fn main() {
    try_init_tracing(TracingConfig::default().default_directive("info,sqlx=warn")).expect("failed to set up logging");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let host: std::net::IpAddr = std::env::var("HOST")
        .ok()
        .and_then(|h| h.parse().ok())
        .unwrap_or(std::net::Ipv4Addr::LOCALHOST.into());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    database::connect(&database_url)
        .await
        .expect("failed to connect to the database");

    NestFactory::create::<AppModule>()
        .set_listen_ip(host)
        .listen_graceful(port)
        .await;
}
