use nestrs::prelude::*;

#[derive(Default)]
#[injectable]
pub struct AppService;

impl AppService {
    pub fn get_hello(&self) -> &'static str {
        "Hello, nestrs!"
    }
}
