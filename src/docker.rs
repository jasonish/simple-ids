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
