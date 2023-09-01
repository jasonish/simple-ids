// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::{bail, Result};
use std::process::{Command, Output};

fn output(args: &[&str]) -> std::io::Result<Output> {
    Command::new("docker").args(args).output()
}

fn parse_json_output(args: &[&str]) -> Result<serde_json::Value> {
    let output = Command::new("docker").args(args).output()?;
    if !output.stderr.is_empty() {
        bail!(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn running(name: &str) -> bool {
    if let Ok(output) = parse_json_output(&["inspect", name]) {
        if let Some(running) = &output[0]["State"]["Running"].as_bool() {
            return *running;
        }
    }
    return false;
}

pub fn image_exists(name: &str) -> bool {
    if let Ok(output) = output(&["image", "inspect", name]) {
        return output.status.success();
    }
    return false;
}

pub fn version() -> Result<String> {
    let output = parse_json_output(&["version", "--format", "{{json .}}"])?;
    if let Some(version) = &output["Server"]["Version"].as_str() {
        return Ok(version.to_string());
    }
    bail!("failed to parse version")
}
