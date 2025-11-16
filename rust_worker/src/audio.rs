use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use tracing::info;

use crate::{config::Config, JobPayload};

pub async fn resample_audio_native(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Resampling audio using ffmpeg-next");
    
    let target_rate = job.params.get("sample_rate")
        .and_then(|v| v.as_u64())
        .unwrap_or(44100) as u32;
    
    // Open input
    let mut ictx = ffmpeg::format::input(&job.input_path)?;
    
    let input_stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .context("No audio stream found")?;
    
    let audio_stream_index = input_stream.index();
    
    let context_decoder = ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = context_decoder.decoder().audio()?;
    
    info!("Resampling from {} Hz to {} Hz", decoder.rate(), target_rate);
    
    // Create resampler
    let mut resampler = ffmpeg::software::resampling::context::Context::get(
        decoder.format(),
        decoder.channel_layout(),
        decoder.rate(),
        decoder.format(),
        decoder.channel_layout(),
        target_rate,
    )?;
    
    // Create output
    let mut octx = ffmpeg::format::output(&job.output_path)?;
    
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::MP3)
        .or_else(|| ffmpeg::encoder::find(ffmpeg::codec::Id::AAC))
        .context("No suitable audio encoder found")?;
    
    let mut ost = octx.add_stream(codec)?;
    let mut encoder = ost.codec().encoder().audio()?;
    
    encoder.set_rate(target_rate as i32);
    encoder.set_channel_layout(decoder.channel_layout());
    encoder.set_channels(decoder.channels());
    encoder.set_format(decoder.format());
    encoder.set_bit_rate(decoder.bit_rate());
    encoder.set_time_base((1, target_rate as i32));
    
    let encoder = encoder.open_as(codec)?;
    ost.set_parameters(&encoder);
    
    octx.write_header()?;
    
    // Process audio
    let mut frame_count = 0;
    
    for (stream, packet) in ictx.packets() {
        if stream.index() == audio_stream_index {
            decoder.send_packet(&packet)?;
            
            let mut decoded = ffmpeg::util::frame::audio::Audio::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
                let mut resampled = ffmpeg::util::frame::audio::Audio::empty();
                
                if let Some(resampled_frame) = resampler.run(&decoded, &mut resampled)? {
                    encoder.send_frame(&resampled_frame)?;
                    
                    let mut encoded = ffmpeg::Packet::empty();
                    while encoder.receive_packet(&mut encoded).is_ok() {
                        encoded.set_stream(0);
                        encoded.rescale_ts(input_stream.time_base(), ost.time_base());
                        encoded.write_interleaved(&mut octx)?;
                    }
                }
                
                frame_count += 1;
                if frame_count % 1000 == 0 {
                    info!("Processed {} audio frames", frame_count);
                }
            }
        }
    }
    
    // Flush resampler
    if let Some(resampled) = resampler.flush()? {
        encoder.send_frame(&resampled)?;
    }
    
    // Flush encoder
    encoder.send_eof()?;
    let mut encoded = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut encoded).is_ok() {
        encoded.set_stream(0);
        encoded.write_interleaved(&mut octx)?;
    }
    
    octx.write_trailer()?;
    
    info!("Resampling complete: {} frames", frame_count);
    Ok(job.output_path.clone())
}

/// Extract audio from video using ffmpeg-next
pub async fn extract_audio_native(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Extracting audio using ffmpeg-next");
    
    let bitrate = job.params.get("bitrate")
        .and_then(|v| v.as_str())
        .unwrap_or("192k");
    
    let bitrate_value = parse_bitrate(bitrate)?;
    
    // Open input
    let mut ictx = ffmpeg::format::input(&job.input_path)?;
    
    let input_stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .context("No audio stream found")?;
    
    let audio_stream_index = input_stream.index();
    
    let context_decoder = ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = context_decoder.decoder().audio()?;
    
    // Create output
    let mut octx = ffmpeg::format::output(&job.output_path)?;
    
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::MP3)
        .or_else(|| ffmpeg::encoder::find(ffmpeg::codec::Id::AAC))
        .context("No suitable audio encoder found")?;
    
    let mut ost = octx.add_stream(codec)?;
    let mut encoder = ost.codec().encoder().audio()?;
    
    encoder.set_rate(decoder.rate() as i32);
    encoder.set_channel_layout(decoder.channel_layout());
    encoder.set_channels(decoder.channels());
    encoder.set_format(codec.audio()?.formats().unwrap().next().unwrap());
    encoder.set_bit_rate(bitrate_value);
    encoder.set_time_base((1, decoder.rate() as i32));
    
    let encoder = encoder.open_as(codec)?;
    ost.set_parameters(&encoder);
    
    octx.write_header()?;
    
    // Process audio
    let mut frame_count = 0;
    
    for (stream, packet) in ictx.packets() {
        if stream.index() == audio_stream_index {
            decoder.send_packet(&packet)?;
            
            let mut decoded = ffmpeg::util::frame::audio::Audio::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
                encoder.send_frame(&decoded)?;
                
                let mut encoded = ffmpeg::Packet::empty();
                while encoder.receive_packet(&mut encoded).is_ok() {
                    encoded.set_stream(0);
                    encoded.rescale_ts(input_stream.time_base(), ost.time_base());
                    encoded.write_interleaved(&mut octx)?;
                }
                
                frame_count += 1;
            }
        }
    }
    
    // Flush encoder
    encoder.send_eof()?;
    let mut encoded = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut encoded).is_ok() {
        encoded.set_stream(0);
        encoded.write_interleaved(&mut octx)?;
    }
    
    octx.write_trailer()?;
    
    info!("Audio extraction complete: {} frames", frame_count);
    Ok(job.output_path.clone())
}

/// Get audio information
pub async fn get_audio_info_native(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Getting audio info using ffmpeg-next");
    
    let ictx = ffmpeg::format::input(&job.input_path)?;
    
    let audio_stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .context("No audio stream found")?;
    
    let context = ffmpeg::codec::context::Context::from_parameters(audio_stream.parameters())?;
    let decoder = context.decoder().audio()?;
    
    let info = serde_json::json!({
        "codec": decoder.codec().map(|c| c.name()).unwrap_or("unknown"),
        "sample_rate": decoder.rate(),
        "channels": decoder.channels(),
        "channel_layout": format!("{:?}", decoder.channel_layout()),
        "format": format!("{:?}", decoder.format()),
        "bit_rate": decoder.bit_rate(),
        "duration": ictx.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE),
    });
    
    std::fs::write(&job.output_path, serde_json::to_string_pretty(&info)?)?;
    
    Ok(job.output_path.clone())
}

/// Generate waveform data from audio
pub async fn generate_waveform_native(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Generating waveform using ffmpeg-next");
    
    let samples = job.params.get("samples")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000) as usize;
    
    // Open input
    let mut ictx = ffmpeg::format::input(&job.input_path)?;
    
    let input_stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .context("No audio stream found")?;
    
    let audio_stream_index = input_stream.index();
    
    let context_decoder = ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = context_decoder.decoder().audio()?;
    
    let mut all_samples: Vec<f32> = Vec::new();
    
    // Decode all audio
    for (stream, packet) in ictx.packets() {
        if stream.index() == audio_stream_index {
            decoder.send_packet(&packet)?;
            
            let mut decoded = ffmpeg::util::frame::audio::Audio::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
                // Extract samples (assuming planar f32 format)
                let data = decoded.data(0);
                let sample_count = decoded.samples();
                
                for i in 0..sample_count {
                    let offset = i * 4; // 4 bytes per f32
                    if offset + 4 <= data.len() {
                        let sample_bytes = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
                        let sample = f32::from_le_bytes(sample_bytes);
                        all_samples.push(sample.abs());
                    }
                }
            }
        }
    }
    
    // Downsample to requested number of samples
    let step = if all_samples.len() > samples {
        all_samples.len() / samples
    } else {
        1
    };
    
    let waveform: Vec<f32> = all_samples
        .chunks(step)
        .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
        .take(samples)
        .collect();
    
    let json = serde_json::to_string(&waveform)?;
    std::fs::write(&job.output_path, json)?;
    
    info!("Generated waveform with {} samples", waveform.len());
    Ok(job.output_path.clone())
}

/// Mix multiple audio tracks
pub async fn mix_audio_native(job: &JobPayload, _config: &Config) -> Result<String> {
    info!("Mixing audio tracks using ffmpeg-next");
    
    let input_files = job.params.get("input_files")
        .and_then(|v| v.as_array())
        .context("input_files array parameter required")?;
    
    if input_files.is_empty() {
        anyhow::bail!("At least one input file required");
    }
    
    // Open all input files
    let mut inputs: Vec<ffmpeg::format::context::Input> = Vec::new();
    let mut decoders: Vec<ffmpeg::decoder::Audio> = Vec::new();
    
    for file in input_files {
        if let Some(path) = file.as_str() {
            let ictx = ffmpeg::format::input(path)?;
            let stream = ictx
                .streams()
                .best(ffmpeg::media::Type::Audio)
                .context("No audio stream found")?;
            
            let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
            let decoder = context.decoder().audio()?;
            
            decoders.push(decoder);
            inputs.push(ictx);
        }
    }
    
    // Use first decoder's properties for output
    let reference_decoder = &decoders[0];
    
    // Create output
    let mut octx = ffmpeg::format::output(&job.output_path)?;
    
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::MP3)
        .or_else(|| ffmpeg::encoder::find(ffmpeg::codec::Id::AAC))
        .context("No suitable audio encoder found")?;
    
    let mut ost = octx.add_stream(codec)?;
    let mut encoder = ost.codec().encoder().audio()?;
    
    encoder.set_rate(reference_decoder.rate() as i32);
    encoder.set_channel_layout(reference_decoder.channel_layout());
    encoder.set_channels(reference_decoder.channels());
    encoder.set_format(codec.audio()?.formats().unwrap().next().unwrap());
    encoder.set_bit_rate(reference_decoder.bit_rate());
    encoder.set_time_base((1, reference_decoder.rate() as i32));
    
    let encoder = encoder.open_as(codec)?;
    ost.set_parameters(&encoder);
    
    octx.write_header()?;
    
    info!("Mixing {} audio tracks", input_files.len());
    
    // Note: Actual mixing would require more complex sample-level processing
    // This is a simplified version that concatenates rather than mixes
    // For true mixing, you'd need to decode all tracks simultaneously and sum samples
    
    for (idx, mut input) in inputs.into_iter().enumerate() {
        info!("Processing track {}/{}", idx + 1, input_files.len());
        
        let stream_index = input
            .streams()
            .best(ffmpeg::media::Type::Audio)
            .unwrap()
            .index();
        
        for (stream, packet) in input.packets() {
            if stream.index() == stream_index {
                decoders[idx].send_packet(&packet)?;
                
                let mut decoded = ffmpeg::util::frame::audio::Audio::empty();
                while decoders[idx].receive_frame(&mut decoded).is_ok() {
                    encoder.send_frame(&decoded)?;
                    
                    let mut encoded = ffmpeg::Packet::empty();
                    while encoder.receive_packet(&mut encoded).is_ok() {
                        encoded.set_stream(0);
                        encoded.write_interleaved(&mut octx)?;
                    }
                }
            }
        }
    }
    
    // Flush encoder
    encoder.send_eof()?;
    let mut encoded = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut encoded).is_ok() {
        encoded.set_stream(0);
        encoded.write_interleaved(&mut octx)?;
    }
    
    octx.write_trailer()?;
    
    info!("Audio mixing complete");
    Ok(job.output_path.clone())
}

// Helper function
fn parse_bitrate(bitrate: &str) -> Result<usize> {
    let bitrate = bitrate.to_uppercase();
    
    if bitrate.ends_with('K') {
        let num: usize = bitrate.trim_end_matches('K').parse()?;
        Ok(num * 1000)
    } else if bitrate.ends_with('M') {
        let num: usize = bitrate.trim_end_matches('M').parse()?;
        Ok(num * 1_000_000)
    } else {
        Ok(bitrate.parse()?)
    }
}
