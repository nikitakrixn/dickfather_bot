mod commander;
mod loader;
mod config;
mod models;

use crate::loader::run;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    pretty_env_logger::init();
    log::info!("Bot started");

    run().await.expect("TODO: panic message");
}

