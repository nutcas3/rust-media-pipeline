use anyhow::{Context, Result};
use sha2::Digest;
use std::fs::{self, File};
use std::io::{Read, Write};
use tracing::info;

use crate::{config::Config, JobPayload};

pub async fn download_file(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Downloading file from URL");
    
    let url = job.params.get("url")
        .and_then(|v| v.as_str())
        .context("url parameter required")?;
    
    let output = Command::new("curl")
        .args(&[
            "-L",
            "-o", &job.output_path,
            url,
        ])
        .output()
        .context("Failed to execute curl")?;
    
    if !output.status.success() {
        anyhow::bail!("Download failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn validate_checksum(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Validating file checksum");
    
    let expected_hash = job.params.get("expected_hash")
        .and_then(|v| v.as_str())
        .context("expected_hash parameter required")?;
    
    // Calculate actual hash
    let mut file = File::open(&job.input_path)
        .context("Failed to open input file")?;
    
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 8192];
    
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    
    let actual_hash = hex::encode(hasher.finalize());
    
    let validation_result = if actual_hash == expected_hash {
        serde_json::json!({
            "valid": true,
            "expected": expected_hash,
            "actual": actual_hash,
            "message": "Checksum validation passed"
        })
    } else {
        serde_json::json!({
            "valid": false,
            "expected": expected_hash,
            "actual": actual_hash,
            "message": "Checksum validation failed"
        })
    };
    
    fs::write(&job.output_path, serde_json::to_string_pretty(&validation_result)?)?;
    
    if actual_hash != expected_hash {
        anyhow::bail!("Checksum mismatch");
    }
    
    Ok(job.output_path.clone())
}

pub async fn probe_media_file(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Probing media file");
    
    let output = Command::new("ffprobe")
        .args(&[
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            "-show_chapters",
            "-show_programs",
            &job.input_path,
        ])
        .output()
        .context("Failed to execute ffprobe")?;
    
    if !output.status.success() {
        anyhow::bail!("FFprobe failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    fs::write(&job.output_path, output.stdout)?;
    
    Ok(job.output_path.clone())
}

pub async fn split_file_chunks(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Splitting file into chunks");
    
    let chunk_size = job.params.get("chunk_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(10 * 1024 * 1024);
    
    let mut input_file = File::open(&job.input_path)
        .context("Failed to open input file")?;
    
    let mut buffer = vec![0u8; chunk_size as usize];
    let mut chunk_index = 0;
    let mut chunk_paths = Vec::new();
    
    loop {
        let bytes_read = input_file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        
        let chunk_path = format!("{}_{:04}", job.output_path, chunk_index);
        let mut chunk_file = File::create(&chunk_path)?;
        chunk_file.write_all(&buffer[..bytes_read])?;
        
        chunk_paths.push(chunk_path);
        chunk_index += 1;
    }
    
    let manifest = serde_json::json!({
        "original_file": job.input_path,
        "chunk_count": chunk_index,
        "chunk_size": chunk_size,
        "chunks": chunk_paths
    });
    
    fs::write(&job.output_path, serde_json::to_string_pretty(&manifest)?)?;
    
    Ok(job.output_path.clone())
}

pub async fn merge_file_chunks(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Merging file chunks");
    
    let chunk_files = job.params.get("chunk_files")
        .and_then(|v| v.as_array())
        .context("chunk_files array parameter required")?;
    
    let mut output_file = File::create(&job.output_path)?;
    
    for chunk in chunk_files {
        if let Some(chunk_path) = chunk.as_str() {
            let mut chunk_file = File::open(chunk_path)
                .context(format!("Failed to open chunk: {}", chunk_path))?;
            
            let mut buffer = Vec::new();
            chunk_file.read_to_end(&mut buffer)?;
            output_file.write_all(&buffer)?;
        }
    }
    
    Ok(job.output_path.clone())
}

pub async fn sanitize_filename(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Sanitizing filename");
    
    let filename = job.params.get("filename")
        .and_then(|v| v.as_str())
        .context("filename parameter required")?;
    
    // Remove or replace unsafe characters
    let sanitized = filename
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
        .replace("  ", " ")
        .trim()
        .to_string();
    
    let result = serde_json::json!({
        "original": filename,
        "sanitized": sanitized,
        "safe": sanitized != filename
    });
    
    fs::write(&job.output_path, serde_json::to_string_pretty(&result)?)?;
    
    Ok(job.output_path.clone())
}

pub async fn create_file_manifest(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Creating file manifest");
    
    let metadata = fs::metadata(&job.input_path)
        .context("Failed to read file metadata")?;
    
    // Calculate hash
    let mut file = File::open(&job.input_path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 8192];
    
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    
    let hash = hex::encode(hasher.finalize());
    
    let manifest = serde_json::json!({
        "file_path": job.input_path,
        "size_bytes": metadata.len(),
        "sha256": hash,
        "created": chrono::DateTime::<chrono::Utc>::from(metadata.created()?).to_rfc3339(),
        "modified": chrono::DateTime::<chrono::Utc>::from(metadata.modified()?).to_rfc3339(),
        "is_readonly": metadata.permissions().readonly(),
    });
    
    fs::write(&job.output_path, serde_json::to_string_pretty(&manifest)?)?;
    
    Ok(job.output_path.clone())
}

pub async fn verify_file_integrity(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Verifying file integrity using native ffmpeg");
    
    let file_type = job.params.get("file_type")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    
    let result = match file_type {
        "video" | "audio" | "auto" => {
            match ffmpeg::format::input(&job.input_path) {
                Ok(mut ictx) => {
                    let mut frame_count = 0;
                    let mut has_video = false;
                    let mut has_audio = false;
                    let mut errors = Vec::new();
                    
                    for stream in ictx.streams() {
                        match stream.parameters().medium() {
                            ffmpeg::media::Type::Video => has_video = true,
                            ffmpeg::media::Type::Audio => has_audio = true,
                            _ => {}
                        }
                    }
                    
                    if has_video {
                        if let Some(video_stream) = ictx.streams().best(ffmpeg::media::Type::Video) {
                            let video_stream_index = video_stream.index();
                            
                            match ffmpeg::codec::context::Context::from_parameters(video_stream.parameters()) {
                                Ok(context_decoder) => {
                                    match context_decoder.decoder().video() {
                                        Ok(mut decoder) => {
                                            for (stream, packet) in ictx.packets() {
                                                if stream.index() == video_stream_index {
                                                    if decoder.send_packet(&packet).is_ok() {
                                                        let mut decoded = ffmpeg::util::frame::video::Video::empty();
                                                        while decoder.receive_frame(&mut decoded).is_ok() {
                                                            frame_count += 1;
                                                            if frame_count >= 10 {
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    if frame_count >= 10 {
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => errors.push(format!("Video decoder error: {}", e)),
                                    }
                                }
                                Err(e) => errors.push(format!("Context error: {}", e)),
                            }
                        }
                    }
                    
                    let is_valid = errors.is_empty() && (has_video || has_audio);
                    
                    serde_json::json!({
                        "valid": is_valid,
                        "file_type": if has_video && has_audio { "video+audio" } 
                                     else if has_video { "video" } 
                                     else if has_audio { "audio" } 
                                     else { "unknown" },
                        "has_video": has_video,
                        "has_audio": has_audio,
                        "frames_tested": frame_count,
                        "errors": if errors.is_empty() { None } else { Some(errors) },
                        "message": if is_valid { "File is valid" } else { "File has errors" }
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "valid": false,
                        "file_type": "invalid",
                        "errors": Some(vec![format!("Cannot open file: {}", e)]),
                        "message": "File cannot be opened or is corrupted"
                    })
                }
            }
        }
        _ => {
            let mut file = File::open(&job.input_path)?;
            let mut buffer = [0u8; 1024];
            let bytes_read = file.read(&mut buffer)?;
            
            serde_json::json!({
                "valid": bytes_read > 0,
                "file_type": "binary",
                "message": "File is readable"
            })
        }
    };
    
    fs::write(&job.output_path, serde_json::to_string_pretty(&result)?)?;
    
    Ok(job.output_path.clone())
}
