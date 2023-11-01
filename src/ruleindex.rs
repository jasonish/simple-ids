// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(crate) struct RuleIndex {
    #[serde(rename = "version")]
    pub _version: u8,
    pub sources: HashMap<String, RuleSource>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuleSource {
    pub summary: String,
    pub obsolete: Option<String>,
    pub parameters: Option<HashMap<String, serde_yaml::Value>>,
}
