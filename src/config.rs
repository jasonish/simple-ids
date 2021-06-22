// Copyright (c) 2021 Jason Ish
//
// Permission is hereby granted, free of charge, to any person
// obtaining a copy of this software and associated documentation
// files (the "Software"), to deal in the Software without
// restriction, including without limitation the rights to use, copy,
// modify, merge, publish, distribute, sublicense, and/or sell copies
// of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT.  IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
// HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
// WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

const DEFAULT_FILENAME: &str = "easy-suricata.json";

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
        let config = serde_json::to_string(self)?;
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
        let config = serde_json::from_str(buf)?;
        Ok(config)
    }
}
