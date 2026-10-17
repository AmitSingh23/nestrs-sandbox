use std::sync::OnceLock;

use nestrs::sea_orm::DbConn;
use sea_orm::{Database, DbErr};

static CONNECTION: OnceLock<DbConn> = OnceLock::new();

/// Opens the Postgres pool. Must run before `AppModule` is built.
pub async fn connect(database_url: &str) -> Result<(), DbErr> {
    let conn = Database::connect(database_url).await?;
    CONNECTION
        .set(conn)
        .map_err(|_| DbErr::Custom("database already connected".into()))
}

pub fn connection() -> DbConn {
    CONNECTION
        .get()
        .expect("database::connect must run before the app is created")
        .clone()
}
