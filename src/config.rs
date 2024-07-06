// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::io::{Read, Write};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::prelude::*;

const YAML_FILENAME: &str = "simple-ids.yml";
const TOML_FILENAME: &str = "simple-ids.toml";

#[derive(Debug, Default, Deserialize, Serialize, Clone, Eq, PartialEq)]
pub(crate) struct Config {
    pub suricata: SuricataConfig,

    #[serde(default)]
    pub evebox: EveBoxConfig,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, Eq, PartialEq)]
pub(crate) struct SuricataConfig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpf: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Eq, PartialEq)]
pub(crate) struct EveBoxConfig {
    #[serde(rename = "allow-remote")]
    pub allow_remote: bool,
    #[serde(rename = "no-tls", default)]
    pub no_tls: bool,
    #[serde(rename = "no-auth", default)]
    pub no_auth: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
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
        if let Ok(buf) = Self::read_file(TOML_FILENAME) {
            match Self::parse_toml(&buf) {
                Err(err) => {
                    error!("Failed to parse configuration file: {}", err);
                }
                Ok(config) => return config,
            }
        }

        if let Ok(config) = Self::read_file(YAML_FILENAME) {
            match Self::parse_yaml(&config) {
                Err(err) => {
                    error!("Failed to parse configuration file: {}", err);
                }
                Ok(config) => return config,
            }
        }

        Self::default()
    }

    pub(crate) fn save(&self) -> Result<()> {
        let mut file = std::fs::File::create(TOML_FILENAME)?;
        let config = toml::to_string(self)?;
        file.write_all(config.as_bytes())?;

        // Delete YAML_FILENAME if exists.
        if std::fs::metadata(YAML_FILENAME).is_ok() {
            std::fs::remove_file(YAML_FILENAME)?;
        }

        Ok(())
    }

    fn read_file(filename: &str) -> Result<String> {
        let mut file = std::fs::File::open(filename)?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;
        Ok(buffer)
    }

    fn parse_yaml(buf: &str) -> Result<Config> {
        Ok(serde_yaml::from_str(buf)?)
    }

    fn parse_toml(buf: &str) -> Result<Config> {
        Ok(toml::from_str(buf)?)
    }
}
