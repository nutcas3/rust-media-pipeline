use anyhow::{Context, Result};
use sha2::{Sha256, Digest};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::process::Command;
use tracing::info;

use crate::{config::Config, JobPayload};

/// Calculate SHA-256 hash of a file
pub async fn calculate_sha256(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Calculating SHA-256 hash");
    
    let mut file = File::open(&job.input_path)
        .context("Failed to open input file")?;
    
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    
    let hash = hasher.finalize();
    let hash_hex = hex::encode(hash);
    
    // Write hash to output file
    let mut output_file = File::create(&job.output_path)?;
    output_file.write_all(hash_hex.as_bytes())?;
    
    Ok(job.output_path.clone())
}

/// Compress file using zstd or gzip
pub async fn compress_archive(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Compressing archive");
    
    let compression = job.params.get("compression")
        .and_then(|v| v.as_str())
        .unwrap_or("gzip");
    
    match compression {
        "gzip" => {
            let output = Command::new("gzip")
                .args(&["-c", &job.input_path])
                .output()
                .context("Failed to execute gzip")?;
            
            if !output.status.success() {
                anyhow::bail!("Gzip failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            
            fs::write(&job.output_path, output.stdout)?;
        }
        "zstd" => {
            let output = Command::new("zstd")
                .args(&["-c", &job.input_path])
                .output()
                .context("Failed to execute zstd")?;
            
            if !output.status.success() {
                anyhow::bail!("Zstd failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            
            fs::write(&job.output_path, output.stdout)?;
        }
        _ => anyhow::bail!("Unsupported compression type: {}", compression),
    }
    
    Ok(job.output_path.clone())
}

/// Extract EXIF metadata from media files
pub async fn extract_exif_metadata(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Extracting EXIF metadata");
    
    let output = Command::new("exiftool")
        .args(&["-json", &job.input_path])
        .output()
        .context("Failed to execute exiftool")?;
    
    if !output.status.success() {
        anyhow::bail!("Exiftool failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    fs::write(&job.output_path, output.stdout)?;
    
    Ok(job.output_path.clone())
}

/// Safely delete the original input file
pub async fn purge_original_file(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Purging original file");
    
    // Verify the file exists before attempting deletion
    if !std::path::Path::new(&job.input_path).exists() {
        anyhow::bail!("Input file does not exist: {}", job.input_path);
    }
    
    fs::remove_file(&job.input_path)
        .context("Failed to delete file")?;
    
    // Write confirmation to output
    let confirmation = format!("File deleted: {}", job.input_path);
    fs::write(&job.output_path, confirmation.as_bytes())?;
    
    Ok(job.output_path.clone())
}

/// Validate file format compliance
pub async fn validate_format_compliance(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Validating format compliance");
    
    let format_type = job.params.get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("video");
    
    let output = match format_type {
        "video" | "audio" => {
            Command::new("ffprobe")
                .args(&[
                    "-v", "error",
                    "-show_format",
                    "-show_streams",
                    "-print_format", "json",
                    &job.input_path,
                ])
                .output()
                .context("Failed to execute ffprobe")?
        }
        _ => anyhow::bail!("Unsupported format type: {}", format_type),
    };
    
    if !output.status.success() {
        anyhow::bail!("Format validation failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    fs::write(&job.output_path, output.stdout)?;
    
    Ok(job.output_path.clone())
}

/// Chain job trigger - enqueue next job in sequence
pub async fn chain_job_trigger(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Triggering chained job");
    
    let next_task = job.params.get("next_task")
        .and_then(|v| v.as_str())
        .context("next_task parameter required")?;
    
    let next_input = job.params.get("next_input")
        .and_then(|v| v.as_str())
        .unwrap_or(&job.input_path);
    
    let next_output = job.params.get("next_output")
        .and_then(|v| v.as_str())
        .context("next_output parameter required")?;
    
    // Create a trigger file with the next job details
    let trigger_data = serde_json::json!({
        "task": next_task,
        "input_path": next_input,
        "output_path": next_output,
        "params": job.params.get("next_params").unwrap_or(&serde_json::json!({}))
    });
    
    fs::write(&job.output_path, serde_json::to_string_pretty(&trigger_data)?)?;
    
    Ok(job.output_path.clone())
}

/// Report job metrics to monitoring system
pub async fn report_metrics(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Reporting metrics");
    
    let metrics = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "job_id": job.params.get("job_id").unwrap_or(&serde_json::json!("unknown")),
        "task": job.task,
        "input_path": job.input_path,
        "output_path": job.output_path,
        "custom_metrics": job.params.get("metrics").unwrap_or(&serde_json::json!({}))
    });
    
    fs::write(&job.output_path, serde_json::to_string_pretty(&metrics)?)?;
    
    Ok(job.output_path.clone())
}
