// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

const DEFAULT_FILENAME: &str = "easy.yml";

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct EveBoxConfig {
    pub enabled: bool,
    #[serde(rename = "allow-external", default)]
    pub allow_external: bool,
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface: Option<String>,
    #[serde(default)]
    pub evebox: EveBoxConfig,
    #[serde(
        rename = "bpf-filter",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bpf_filter: Option<String>,
    #[serde(rename = "start-on-boot", default)]
    pub start_on_boot: bool,
    #[serde(
        rename = "data-directory",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub data_directory: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        if let Ok(config) = Self::read_file(DEFAULT_FILENAME) {
            match Self::parse(&config) {
                Err(err) => {
                    tracing::error!("Failed to parse configuration file: {}", err);
                }
                Ok(config) => return config,
            }
        }
        return Self::default();
    }

    pub fn save(&self, filename: Option<&str>) -> Result<()> {
        let filename = filename.unwrap_or(DEFAULT_FILENAME);
        let mut file = std::fs::File::create(filename)?;
        let config = serde_yaml::to_string(self)?;
        file.write_all(config.as_bytes())?;
        Ok(())
    }

    pub fn read_file(filename: &str) -> Result<String> {
        let mut file = std::fs::File::open(filename)?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;
        Ok(buffer)
    }

    pub fn parse(buf: &str) -> Result<Config> {
        let config = serde_yaml::from_str(buf)?;
        Ok(config)
    }
}
