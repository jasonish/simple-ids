// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::{bail, Result};
use std::process::{Command, Output};

fn output(args: &[&str]) -> std::io::Result<Output> {
    Command::new("podman").args(args).output()
}

pub fn version() -> Result<String> {
    match Command::new("podman")
        .args(["version", "--format", "{{json .}}"])
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr).to_string();
                bail!(err);
            }
            // Attempt to parse the output info as JSON and get the server version.
            if let Ok(version) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if let Some(version) = version["Client"]["Version"].as_str() {
                    return Ok(version.to_string());
                }
            }
        }
        Err(_err) => {}
    }
    bail!("Failed to find Podman")
}

pub fn parse_json_output(args: &[&str]) -> Result<serde_json::Value> {
    let output = Command::new(args[0]).args(&args[1..]).output()?;
    if !output.stderr.is_empty() {
        bail!(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn running(name: &str) -> bool {
    if let Ok(output) = parse_json_output(&["podman", "inspect", name]) {
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
