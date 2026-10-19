use std::sync::Arc;

use nestrs::prelude::*;
use nestrs::sea_orm::DbConn;

#[injectable]
pub struct AppService {
    db: Arc<DbConn>,
}

impl AppService {
    pub fn get_hello(&self) -> &'static str {
        "Hello, nestrs!"
    }

    pub async fn db_ok(&self) -> bool {
        self.db.ping().await.is_ok()
    }
}
