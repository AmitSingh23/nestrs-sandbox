use std::sync::Arc;

use nestrs::prelude::*;

use crate::app_service::AppService;

#[controller(prefix = "/")]
pub struct AppController;

#[routes(state = AppService)]
impl AppController {
    #[get("/")]
    async fn get_hello(State(service): State<Arc<AppService>>) -> &'static str {
        service.get_hello()
    }
}
