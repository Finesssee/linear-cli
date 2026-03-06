use anyhow::{Context, Result};
use clap::{Subcommand, ValueHint};
use std::io::{self, Write};

use crate::api::{parse_linear_upload_url, LinearClient};

#[cfg(unix)]
fn create_private_file(path: &str) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    Ok(std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("Failed to create file: {}", path))?)
}

#[cfg(not(unix))]
fn create_private_file(path: &str) -> Result<std::fs::File> {
    Ok(std::fs::File::create(path).with_context(|| format!("Failed to create file: {}", path))?)
}

#[derive(Subcommand)]
pub enum UploadCommands {
    /// Fetch an upload from Linear's upload storage
    #[command(alias = "get")]
    Fetch {
        /// The Linear upload URL (e.g., https://uploads.linear.app/...)
        url: String,

        /// Output file path (if not specified, outputs to stdout)
        #[arg(short = 'f', long = "file", value_hint = ValueHint::FilePath)]
        file: Option<String>,
    },
}

pub async fn handle(cmd: UploadCommands) -> Result<()> {
    match cmd {
        UploadCommands::Fetch { url, file } => fetch_upload(&url, file).await,
    }
}

async fn fetch_upload(url: &str, file: Option<String>) -> Result<()> {
    parse_linear_upload_url(url)?;

    let client = LinearClient::new()?;

    if let Some(file_path) = file {
        // Stream directly to file
        let mut file = create_private_file(&file_path)?;
        let bytes_written = client
            .fetch_to_writer(url, &mut file)
            .await
            .context("Failed to fetch upload from Linear")?;
        eprintln!("Downloaded {} bytes to {}", bytes_written, file_path);
    } else {
        // Stream directly to stdout
        let mut stdout_handle = io::stdout().lock();
        let bytes_written = client
            .fetch_to_writer(url, &mut stdout_handle)
            .await
            .context("Failed to fetch upload from Linear")?;
        stdout_handle.flush()?;
        drop(stdout_handle);
        eprintln!("Downloaded {} bytes", bytes_written);
    }

    Ok(())
}
