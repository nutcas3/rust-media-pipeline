use anyhow::{Context, Result};
use std::process::Command;
use tracing::info;

use crate::{config::Config, JobPayload};

/// Normalize audio loudness to EBU R128 standard (-23 LUFS)
pub async fn normalize_loudness(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Normalizing audio loudness");
    
    let target_lufs = job.params.get("target_lufs")
        .and_then(|v| v.as_str())
        .unwrap_or("-23");
    
    // Two-pass normalization using loudnorm filter
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-af", &format!("loudnorm=I={}:TP=-1.5:LRA=11", target_lufs),
            "-ar", "48000",
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

/// Resample audio to a different sample rate
pub async fn resample_audio(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Resampling audio");
    
    let sample_rate = job.params.get("sample_rate")
        .and_then(|v| v.as_u64())
        .unwrap_or(44100);
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-ar", &sample_rate.to_string(),
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

/// Encode audio to MP3 format
pub async fn encode_to_mp3(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Encoding to MP3");
    
    let bitrate = job.params.get("bitrate")
        .and_then(|v| v.as_str())
        .unwrap_or("192k");
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-codec:a", "libmp3lame",
            "-b:a", bitrate,
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

/// Generate waveform data as JSON for UI visualization
pub async fn generate_waveform_json(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Generating waveform JSON");
    
    let samples = job.params.get("samples")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000);
    
    // Extract audio data using ffmpeg
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-ac", "1",
            "-filter:a", &format!("aresample={}", samples),
            "-map", "0:a",
            "-c:a", "pcm_s16le",
            "-f", "data",
            "-",
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Convert raw audio data to JSON array of amplitudes
    let audio_data = output.stdout;
    let mut waveform: Vec<i16> = Vec::new();
    
    for chunk in audio_data.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        waveform.push(sample);
    }
    
    // Downsample if needed
    let step = (waveform.len() / samples as usize).max(1);
    let downsampled: Vec<i16> = waveform.iter().step_by(step).copied().collect();
    
    // Write JSON
    let json = serde_json::to_string(&downsampled)?;
    std::fs::write(&job.output_path, json)?;
    
    Ok(job.output_path.clone())
}

pub async fn extract_mono_track(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Extracting mono track");
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-ac", "1",
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

/// Apply noise reduction to audio
pub async fn reduce_audio_noise(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Reducing audio noise");
    
    let noise_reduction = job.params.get("noise_reduction")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.21); // 0.0 to 1.0
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-af", &format!("anlmdn=s={}:p=0.002:r=0.002:m=15", noise_reduction),
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

pub async fn split_audio_channels(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Splitting audio channels");
    
    // Extract left channel
    let left_output = job.output_path.replace(".mp3", "_left.mp3");
    let left = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-af", "pan=mono|c0=c0",
            "-y",
            &left_output,
        ])
        .output()
        .context("Failed to extract left channel")?;
    
    if !left.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&left.stderr));
    }
    
    // Extract right channel
    let right_output = job.output_path.replace(".mp3", "_right.mp3");
    let right = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-af", "pan=mono|c0=c1",
            "-y",
            &right_output,
        ])
        .output()
        .context("Failed to extract right channel")?;
    
    if !right.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&right.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn detect_audio_format(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Detecting audio format");
    
    let output = Command::new("ffprobe")
        .args(&[
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            &job.input_path,
        ])
        .output()
        .context("Failed to execute ffprobe")?;
    
    if !output.status.success() {
        anyhow::bail!("FFprobe failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    std::fs::write(&job.output_path, output.stdout)?;
    
    Ok(job.output_path.clone())
}

pub async fn detect_audio_peaks(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Detecting audio peaks");
    
    let threshold = job.params.get("threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(-3.0); // dB
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-af", &format!("astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.Peak_level:file={}", job.output_path),
            "-f", "null",
            "-",
        ])
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

/// Remove silence from audio
pub async fn remove_silence(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Removing silence from audio");
    
    let noise_threshold = job.params.get("noise_threshold")
        .and_then(|v| v.as_str())
        .unwrap_or("-50dB");
    
    let duration = job.params.get("duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5); // seconds
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-af", &format!("silenceremove=start_periods=1:start_duration={}:start_threshold={}:detection=peak", duration, noise_threshold),
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

/// Mix multiple audio tracks together
pub async fn mix_audio_tracks(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Mixing audio tracks");
    
    let input_files = job.params.get("input_files")
        .and_then(|v| v.as_array())
        .context("input_files array parameter required")?;
    
    if input_files.is_empty() {
        anyhow::bail!("At least one input file required");
    }
    
    let mut args = vec![];
    for file in input_files {
        if let Some(path) = file.as_str() {
            args.push("-i");
            args.push(path);
        }
    }
    
    let filter = format!("amix=inputs={}:duration=longest", input_files.len());
    args.extend(&["-filter_complex", &filter, "-y", &job.output_path]);
    
    let output = Command::new("ffmpeg")
        .args(&args)
        .output()
        .context("Failed to execute ffmpeg")?;
    
    if !output.status.success() {
        anyhow::bail!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(job.output_path.clone())
}

pub async fn apply_audio_fade(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Applying audio fade");
    
    let fade_in = job.params.get("fade_in")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0); // seconds
    
    let fade_out = job.params.get("fade_out")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0); // seconds
    
    let mut filter = String::new();
    
    if fade_in > 0.0 {
        filter.push_str(&format!("afade=t=in:st=0:d={}", fade_in));
    }
    
    if fade_out > 0.0 {
        if !filter.is_empty() {
            filter.push(',');
        }
        filter.push_str(&format!("afade=t=out:st=0:d={}", fade_out));
    }
    
    if filter.is_empty() {
        anyhow::bail!("At least one fade parameter (fade_in or fade_out) must be specified");
    }
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-af", &filter,
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

pub async fn extract_audio_from_video(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Extracting audio from video");
    
    let format = job.params.get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("mp3");
    
    let bitrate = job.params.get("bitrate")
        .and_then(|v| v.as_str())
        .unwrap_or("192k");
    
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &job.input_path,
            "-vn",
            "-acodec", format,
            "-b:a", bitrate,
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
