use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub redis: RedisConfig,
    pub storage: StorageConfig,
    pub processing: ProcessingConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub queue_name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    #[serde(rename = "type")]
    pub storage_type: String,
    pub input_path: String,
    pub output_path: String,
    #[serde(default)]
    pub s3: S3Config,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProcessingConfig {
    pub max_workers: usize,
    pub timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .context(format!("Failed to read config file: {}", path))?;
        
        let config: Config = toml::from_str(&contents)
            .context("Failed to parse config file")?;
        
        Ok(config)
    }
}
