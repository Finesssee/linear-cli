use anyhow::{Context, Result};
use clap::Subcommand;
use std::io::{self, Write};

use crate::api::LinearClient;

#[derive(Subcommand)]
pub enum UploadCommands {
    /// Fetch an upload from Linear's upload storage
    #[command(alias = "get")]
    Fetch {
        /// The Linear upload URL (e.g., https://uploads.linear.app/...)
        url: String,

        /// Output file path (if not specified, outputs to stdout)
        #[arg(short = 'f', long = "file")]
        file: Option<String>,
    },
}

pub async fn handle(cmd: UploadCommands) -> Result<()> {
    match cmd {
        UploadCommands::Fetch { url, file } => fetch_upload(&url, file).await,
    }
}

async fn fetch_upload(url: &str, file: Option<String>) -> Result<()> {
    // Validate URL is a Linear upload URL
    if !url.starts_with("https://uploads.linear.app/") {
        anyhow::bail!(
            "Invalid URL: expected Linear upload URL starting with 'https://uploads.linear.app/'"
        );
    }

    let client = LinearClient::new()?;

    if let Some(file_path) = file {
        // Stream directly to file
        let mut file = std::fs::File::create(&file_path)
            .with_context(|| format!("Failed to create file: {}", file_path))?;
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
