use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod binary;
mod acquisition;
mod video;
mod audio;
mod config;

use config::Config;

#[derive(Debug, Deserialize, Serialize)]
struct JobPayload {
    task: String,
    input_path: String,
    output_path: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct JobResult {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metrics: Option<JobMetrics>,
}

#[derive(Debug, Serialize)]
struct JobMetrics {
    duration_ms: u64,
    input_size_bytes: u64,
    output_size_bytes: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();
    
    // Initialize FFmpeg
    ffmpeg_video::init_ffmpeg()
        .context("Failed to initialize FFmpeg")?;
    info!("FFmpeg initialized successfully");

    // Load configuration
    let config = Config::load("./config/settings.toml")
        .context("Failed to load configuration")?;

    info!("Rust worker started");

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        error!("Usage: rust_worker <job_payload_json>");
        std::process::exit(1);
    }

    let job_payload_str = &args[1];
    let job: JobPayload = serde_json::from_str(job_payload_str)
        .context("Failed to parse job payload")?;

    info!(task = %job.task, input = %job.input_path, "Processing job");

    let start = std::time::Instant::now();
    
    // Execute the job
    let result = match execute_job(&job, &config).await {
        Ok(output_path) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            
            let input_size = get_file_size(&job.input_path).unwrap_or(0);
            let output_size = get_file_size(&output_path).unwrap_or(0);
            
            JobResult {
                success: true,
                message: format!("Job '{}' completed successfully", job.task),
                output_path: Some(output_path),
                metrics: Some(JobMetrics {
                    duration_ms,
                    input_size_bytes: input_size,
                    output_size_bytes: output_size,
                }),
            }
        }
        Err(e) => {
            error!(error = %e, "Job failed");
            JobResult {
                success: false,
                message: format!("Job failed: {}", e),
                output_path: None,
                metrics: None,
            }
        }
    };

    // Output result as JSON
    println!("{}", serde_json::to_string(&result)?);

    if result.success {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

async fn execute_job(job: &JobPayload, config: &Config) -> Result<String> {
    match job.task.as_str() {
        "download_file" => acquisition::download_file(job, config).await,
        "validate_checksum" => acquisition::validate_checksum(job, config).await,
        "probe_media_file" => acquisition::probe_media_file(job, config).await,
        "split_file_chunks" => acquisition::split_file_chunks(job, config).await,
        "merge_file_chunks" => acquisition::merge_file_chunks(job, config).await,
        "sanitize_filename" => acquisition::sanitize_filename(job, config).await,
        "create_file_manifest" => acquisition::create_file_manifest(job, config).await,
        "verify_file_integrity" => acquisition::verify_file_integrity(job, config).await,
        
        "transcode_h264_to_h265" => ffmpeg_video::transcode_video_native(job, config).await,
        "resize_to_720p" => ffmpeg_video::resize_video_native(job, config).await,
        "get_video_info" => ffmpeg_video::get_video_info_native(job, config).await,
        "extract_frames" => ffmpeg_video::extract_frames_native(job, config).await,
        "extract_thumbnails" => ffmpeg_video::extract_thumbnails(job, config).await,
        "create_animated_gif" => ffmpeg_video::create_animated_gif(job, config).await,
        "detect_scene_cuts" => ffmpeg_video::detect_scene_cuts(job, config).await,
        "apply_watermark" => ffmpeg_video::apply_watermark(job, config).await,
        "extract_key_frame" => ffmpeg_video::extract_key_frame(job, config).await,
        
        "resample_audio" => ffmpeg_audio::resample_audio_native(job, config).await,
        "extract_audio_from_video" => ffmpeg_audio::extract_audio_native(job, config).await,
        "get_audio_info" => ffmpeg_audio::get_audio_info_native(job, config).await,
        "generate_waveform_json" => ffmpeg_audio::generate_waveform_native(job, config).await,
        "mix_audio_tracks" => ffmpeg_audio::mix_audio_native(job, config).await,
        
        "calculate_sha256" => binary::calculate_sha256(job, config).await,
        "compress_archive" => binary::compress_archive(job, config).await,
        "extract_exif_metadata" => binary::extract_exif_metadata(job, config).await,
        "purge_original_file" => binary::purge_original_file(job, config).await,
        "validate_format_compliance" => binary::validate_format_compliance(job, config).await,
        "chain_job_trigger" => binary::chain_job_trigger(job, config).await,
        "report_metrics" => binary::report_metrics(job, config).await,
        
        _ => {
            warn!(task = %job.task, "Unknown task type");
            anyhow::bail!("Unknown task type: {}", job.task)
        }
    }
}

fn get_file_size(path: &str) -> Result<u64> {
    let metadata = fs::metadata(path)?;
    Ok(metadata.len())
}
