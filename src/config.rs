// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::io::{Read, Write};

use anyhow::Result;
use serde::{Deserialize, Serialize};

const FILENAME: &str = "simple-ids.yml";

#[derive(Debug, Default, Deserialize, Serialize, Clone, Eq, PartialEq)]
pub(crate) struct Config {
    pub suricata: SuricataConfig,

    #[serde(default)]
    pub evebox: EveBoxConfig,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, Eq, PartialEq)]
pub(crate) struct SuricataConfig {
    pub interfaces: Vec<String>,
    pub image: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Eq, PartialEq)]
pub(crate) struct EveBoxConfig {
    #[serde(rename = "allow-remote")]
    pub allow_remote: bool,
    #[serde(rename = "no-tls", default)]
    pub no_tls: bool,
    #[serde(rename = "no-auth", default)]
    pub no_auth: bool,
    pub image: Option<String>,
}

impl Default for EveBoxConfig {
    fn default() -> Self {
        Self {
            allow_remote: false,
            no_tls: true,
            no_auth: true,
            image: None,
        }
    }
}

impl Config {
    pub(crate) fn new() -> Self {
        if let Ok(config) = Self::read_file(FILENAME) {
            match Self::parse(&config) {
                Err(err) => {
                    tracing::error!("Failed to parse configuration file: {}", err);
                }
                Ok(config) => return config,
            }
        }
        Self::default()
    }

    pub(crate) fn save(&self) -> Result<()> {
        let mut file = std::fs::File::create(FILENAME)?;
        let config = serde_yaml::to_string(self)?;
        file.write_all(config.as_bytes())?;
        Ok(())
    }

    fn read_file(filename: &str) -> Result<String> {
        let mut file = std::fs::File::open(filename)?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;
        Ok(buffer)
    }

    fn parse(buf: &str) -> Result<Config> {
        let config = serde_yaml::from_str(buf)?;
        Ok(config)
    }
}
