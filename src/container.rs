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

use crate::{docker, podman};
use anyhow::{anyhow, bail, Result};
use std::ffi::OsStr;
use std::process::Command;
use tracing::debug;

#[derive(PartialEq)]
pub enum ContainerRuntime {
    Docker,
    Podman,
}

impl ContainerRuntime {
    pub fn image_exists(&self, name: &str) -> bool {
        match self {
            Self::Docker => crate::docker::image_exists(name),
            Self::Podman => crate::podman::image_exists(name),
        }
    }

    pub fn program_name(&self) -> &str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        }
    }

    /// Wrapper around calling .output on the container.
    ///
    /// If an error occurs calling the executable, that error is returned. Otherwise the
    /// exit status is checked an error is returned containing stderr. On success, stdout will
    /// be returned as a String.
    pub fn exec_output<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new(self.program_name())
            .args(args)
            .output()
            .map_err(|err| anyhow!("{}: {}", self.program_name(), err))?;
        if !output.status.success() {
            bail!(String::from_utf8_lossy(&output.stderr).trim().to_string())
        } else {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
    }

    pub fn exec_output_stderr<I, S>(&self, args: I) -> Result<(String, String)>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new(self.program_name())
            .args(args)
            .output()
            .map_err(|err| anyhow!("{}: {}", self.program_name(), err))?;
        if !output.status.success() {
            bail!(String::from_utf8_lossy(&output.stderr).trim().to_string())
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Ok((stdout, stderr))
        }
    }

    /// Wrapper around .status that will error out on non-zero exit codes.
    pub fn exec_status<I, S>(&self, args: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let status = Command::new(self.program_name()).args(args).status()?;
        if !status.success() {
            bail!(status.to_string())
        } else {
            Ok(())
        }
    }

    pub fn container_running(&self, name: &str) -> bool {
        match self {
            Self::Docker => docker::running(name),
            Self::Podman => podman::running(name),
        }
    }

    pub fn last_log_line(&self, name: &str) -> Option<String> {
        if let Ok((stdout, stderr)) = self.exec_output_stderr(&["logs", "--tail=1", name]) {
            if !stderr.is_empty() {
                return Some(stderr.trim().to_string());
            }
            if !stdout.is_empty() {
                return Some(stdout.trim().to_string());
            }
        }
        None
    }

    pub fn requires_privilege(&self) -> bool {
        match self {
            Self::Docker => {
                // Docker requires privilege on Raspberry Pi OS.
                if let Ok(os_info) = os_release::OsRelease::new() {
                    // Raspberry Pi OS requires privilege for Docker at this time.
                    if os_info.id.to_lowercase().contains("rasp")
                        || os_info.name.to_lowercase().contains("rasp")
                        || os_info.pretty_name.to_lowercase().contains("rasp")
                    {
                        return true;
                    }
                }
            }
            Self::Podman => {
                // Podman nevers requires privilege as far as I can tell.
            }
        }
        false
    }
}

pub fn find_runtime(podman_only: bool) -> Option<ContainerRuntime> {
    if !podman_only {
        debug!("Checking for Docker container runtime");
        if let Ok(_version) = docker::version() {
            return Some(ContainerRuntime::Docker);
        }
    }
    debug!("Checking for Podman container runtime");
    if let Ok(_version) = podman::version() {
        return Some(ContainerRuntime::Podman);
    }
    None
}
