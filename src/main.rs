mod config;
mod cover;
mod downloader;
mod loudness;
mod native_host;
mod whisper;
mod worker;

use anyhow::Result;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 3 && args[1] == "worker" {
        let task_file = PathBuf::from(&args[2]);
        worker::run_worker(task_file).await?;
    } else {
        native_host::run_native_host()?;
    }

    Ok(())
}
