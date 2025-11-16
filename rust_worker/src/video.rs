use anyhow::{Context, Result};
use std::process::Command;
use tracing::{info, warn};

use crate::{config::Config, JobPayload};

pub async fn transcode_h264_to_h265(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Transcoding H.264 to H.265");
    
    let bitrate = job.params.get("bitrate")
        .and_then(|v| v.as_str())
        .unwrap_or("1M");
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-c:v", "libx265",
            "-b:v", bitrate,
            "-c:a", "copy",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn resize_to_720p(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Resizing video to 720p");
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vf", "scale=-2:720",
            "-c:a", "copy",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn extract_thumbnails(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Extracting thumbnails");
    
    let count = job.params.get("count")
        .and_then(|v| v.as_u64())
        .unwrap_or(10);
    
    // Extract thumbnails using fps filter
    let fps = format!("fps=1/{}", count);
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vf", &fps,
            "-y",
            &format!("{}_%04d.jpg", job.output_path),
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn create_animated_gif(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Creating animated GIF");
    
    let duration = job.params.get("duration")
        .and_then(|v| v.as_u64())
        .unwrap_or(5);
    
    let fps = job.params.get("fps")
        .and_then(|v| v.as_u64())
        .unwrap_or(10);
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-t", &duration.to_string(),
            "-vf", &format!("fps={},scale=480:-1:flags=lanczos", fps),
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn detect_scene_cuts(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Detecting scene cuts");
    
    let threshold = job.params.get("threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.3);
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vf", &format!("select='gt(scene,{})',metadata=print:file={}", threshold, job.output_path),
            "-f", "null",
            "-",
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        warn!("FFmpeg scene detection had issues: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn apply_watermark(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Applying watermark");
    
    let watermark_path = job.params.get("watermark_path")
        .and_then(|v| v.as_str())
        .context("watermark_path parameter required")?;
    
    let position = job.params.get("position")
        .and_then(|v| v.as_str())
        .unwrap_or("10:10"); // top-left corner with 10px padding
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-i", watermark_path,
            "-filter_complex", &format!("overlay={}", position),
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn extract_key_frame(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Extracting key frame");
    
    let timestamp = job.params.get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("00:00:01");
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-ss", timestamp,
            "-i", &job.input_path,
            "-vframes", "1",
            "-q:v", "2",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn burn_in_subtitles(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Burning in subtitles");
    
    let subtitle_path = job.params.get("subtitle_path")
        .and_then(|v| v.as_str())
        .context("subtitle_path parameter required")?;
    
    // Escape the subtitle path for FFmpeg filter
    let escaped_path = subtitle_path.replace("\\", "\\\\").replace(":", "\\:");
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vf", &format!("subtitles={}", escaped_path),
            "-c:a", "copy",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

/// Rotate video by specified degrees (90, 180, 270)
pub async fn rotate_video(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Rotating video");
    
    let degrees = job.params.get("degrees")
        .and_then(|v| v.as_u64())
        .unwrap_or(90);
    
    let transpose = match degrees {
        90 => "1",      // 90 clockwise
        180 => "2,transpose=2", // 180
        270 => "2",     // 90 counter-clockwise
        _ => "1",
    };
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vf", &format!("transpose={}", transpose),
            "-c:a", "copy",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn stabilize_video(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Stabilizing video");
    
    let shakiness = job.params.get("shakiness")
        .and_then(|v| v.as_u64())
        .unwrap_or(5); // 1-10, higher = more shaky
    
    let smoothing = job.params.get("smoothing")
        .and_then(|v| v.as_u64())
        .unwrap_or(10); // Higher = smoother
    
    // Two-pass stabilization
    let transforms_file = format!("{}.trf", job.output_path);
    
    // Pass 1: Detect
    let detect = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vf", &format!("vidstabdetect=shakiness={}:result={}", shakiness, transforms_file),
            "-f", "null",
            "-",
        ])
        .output()
        .context("Failed to execute ffmpeg detect pass")?;
    
    if !detect.status.success() {
        anyhow::bail!("FFmpeg detect failed: {}", String::from_utf8_lossy(&detect.stderr));
    }
    
    // Pass 2: Transform
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vf", &format!("vidstabtransform=smoothing={}:input={}", smoothing, transforms_file),
            "-c:a", "copy",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg transform pass")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg transform failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Cleanup transforms file
    std::fs::remove_file(&transforms_file).ok();
    
    Ok(job.output_path.clone())
}

pub async fn deinterlace_video(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Deinterlacing video");
    
    let method = job.params.get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("yadif"); // yadif, bwdif, or w3fdif
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vf", method,
            "-c:a", "copy",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

/// Apply color grading/correction to video
pub async fn color_grade_video(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Applying color grading");
    
    let brightness = job.params.get("brightness")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0); // -1.0 to 1.0
    
    let contrast = job.params.get("contrast")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0); // 0.0 to 2.0
    
    let saturation = job.params.get("saturation")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0); // 0.0 to 3.0
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vf", &format!("eq=brightness={}:contrast={}:saturation={}", brightness, contrast, saturation),
            "-c:a", "copy",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

/// Change video playback speed
pub async fn change_video_speed(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Changing video speed");
    
    let speed = job.params.get("speed")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0); // 0.5 = half speed, 2.0 = double speed
    
    let video_pts = 1.0 / speed;
    let audio_tempo = speed;
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-filter_complex", &format!("[0:v]setpts={}*PTS[v];[0:a]atempo={}[a]", video_pts, audio_tempo),
            "-map", "[v]",
            "-map", "[a]",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn concatenate_videos(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Concatenating videos");
    
    let input_files = job.params.get("input_files")
        .and_then(|v| v.as_array())
        .context("input_files array parameter required")?;
    
    // Create concat file list
    let concat_file = format!("{}.txt", job.output_path);
    let mut concat_content = String::new();
    
    for file in input_files {
        if let Some(path) = file.as_str() {
            concat_content.push_str(&format!("file '{}'\n", path));
        }
    }
    
    std::fs::write(&concat_file, concat_content)
        .context("Failed to write concat file")?;
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-f", "concat",
            "-safe", "0",
            "-i", &concat_file,
            "-c", "copy",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Cleanup concat file
    std::fs::remove_file(&concat_file).ok();
    
    Ok(job.output_path.clone())
}

pub async fn convert_video_format(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Converting video format");
    
    let format = job.params.get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("mp4");
    
    let codec = match format {
        "webm" => "libvpx-vp9",
        "mkv" => "copy",
        "avi" => "libx264",
        _ => "copy",
    };
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-c:v", codec,
            "-c:a", "copy",
            "-y",
            &job.output_path,
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}
