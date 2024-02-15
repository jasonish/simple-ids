// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::{bail, Result};
use serde::Deserialize;
use std::process::Command;
use tracing::{debug, error, info};

use crate::{
    context::Context, EVEBOX_VOLUME_LIB, SURICATA_VOLUME_LIB, SURICATA_VOLUME_LOG,
    SURICATA_VOLUME_RUN,
};

const DEFAULT_SURICATA_IMAGE: &str = "docker.io/jasonish/suricata:latest";
const DEFAULT_EVEBOX_IMAGE: &str = "docker.io/jasonish/evebox:master";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ContainerManager {
    Docker(DockerManager),
    Podman(PodmanManager),
}

impl std::fmt::Display for ContainerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ContainerManager::Docker(_) => "Docker",
            ContainerManager::Podman(_) => "Podman",
        };
        write!(f, "{name}")
    }
}

impl ContainerManager {
    pub(crate) fn command(&self) -> Command {
        Command::new(self.bin())
    }

    pub(crate) fn bin(&self) -> &str {
        match self {
            Self::Docker(docker) => docker.bin(),
            Self::Podman(podman) => podman.bin(),
        }
    }

    /// Test if a container manager exists.
    pub(crate) fn exists(&self) -> bool {
        Command::new(self.bin())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
    }

    /// Return true if the container manager is Podman.
    pub(crate) fn is_podman(&self) -> bool {
        matches!(self, ContainerManager::Podman(_))
    }

    /// Return true if the container manager is Docker.
    pub(crate) fn is_docker(&self) -> bool {
        matches!(self, ContainerManager::Docker(_))
    }

    pub(crate) fn version(&self) -> Result<String> {
        let output = self
            .command()
            .args(["version", "--format", "{{json . }}"])
            .output()?;
        if !output.status.success() {
            bail!(String::from_utf8_lossy(&output.stderr).to_string());
        } else if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(version) = json["Client"]["Version"].as_str() {
                return Ok(version.to_string());
            }
            if let Some(version) = json["Version"].as_str() {
                return Ok(version.to_string());
            }
        }
        bail!(
            "Failed to find {} version in output: {}",
            self.to_string(),
            String::from_utf8_lossy(&output.stdout).to_string()
        );
    }

    /// Quietly remove container.
    pub(crate) fn quiet_rm(&self, name: &str) {
        let mut args = vec!["rm"];

        // Podman needs to be a little more agressive here.
        if self.is_podman() {
            args.push("--force");
        }

        args.push(name);
        let _ = self.command().args(&args).output();
    }

    pub(crate) fn stop(&self, name: &str, signal: Option<&str>) -> Result<()> {
        let mut cmd = self.command();
        cmd.arg("stop");

        // Custom stop signals are not supported on Podman.
        if self.is_docker() {
            cmd.args(["--signal", signal.unwrap_or("SIGTERM")]);
        }
        cmd.arg(name);
        let output = cmd.output()?;
        if !output.status.success() {
            bail!(String::from_utf8_lossy(&output.stderr).to_string());
        }
        Ok(())
    }

    pub(crate) fn pull(&self, image: &str) -> Result<()> {
        let status = self.command().args(["pull", image]).status()?;
        if status.success() {
            Ok(())
        } else {
            bail!("Pull did not exit successfully")
        }
    }

    pub(crate) fn inspect_first(&self, name: &str) -> Result<InspectEntry> {
        let mut command = self.command();
        command.args(["inspect", name]);
        let mut entries: Vec<InspectEntry> = command_json(&mut command)?;
        if entries.is_empty() {
            bail!("{} returned unexpected empty inspect array", self);
        } else {
            Ok(entries.swap_remove(0))
        }
    }

    pub(crate) fn has_image(&self, name: &str) -> bool {
        self.inspect_first(name).is_ok()
    }

    pub(crate) fn is_running(&self, name: &str) -> bool {
        if let Ok(state) = self.state(name) {
            return state.running;
        }
        false
    }

    /// Return the Inspect.State object for a container.
    ///
    /// If the container doesn't exist an error is returned.
    pub(crate) fn state(&self, name: &str) -> Result<InspectState> {
        match self.inspect_first(name)?.state {
            Some(state) => Ok(state),
            None => bail!("not a container"),
        }
    }

    /// Test if a container exists.
    ///
    /// Any failure results in false.
    pub(crate) fn container_exists(&self, name: &str) -> bool {
        if let Ok(output) = self.command().args(["inspect", name]).output() {
            return output.status.success();
        }
        false
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct PodmanManager {}

impl PodmanManager {
    pub(crate) fn new() -> Self {
        Self {}
    }

    pub(crate) fn bin(&self) -> &str {
        "podman"
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct DockerManager {}

impl DockerManager {
    pub(crate) fn new() -> Self {
        Self {}
    }

    pub(crate) fn bin(&self) -> &str {
        "docker"
    }
}

/// Command extensions useful for containers.
pub(crate) trait CommandExt {
    /// Like `Command::output`, but return an error on command failure
    /// as well as non-successful exit code.
    fn status_output(&mut self) -> anyhow::Result<Vec<u8>>;

    /// Like `Command::status` but will also fail if the command did
    /// not exit successfully.
    fn status_ok(&mut self) -> Result<()>;
}

impl CommandExt for std::process::Command {
    fn status_output(&mut self) -> Result<Vec<u8>> {
        let output = self.output()?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            bail!(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    fn status_ok(&mut self) -> Result<()> {
        let status = self.status()?;
        if status.success() {
            Ok(())
        } else {
            bail!("Failed with exit code {:?}", status.code())
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct InspectEntry {
    #[serde(rename = "Id")]
    _id: String,

    // Only found when inspecting containers.
    #[serde(rename = "State")]
    state: Option<InspectState>,

    // Only found when inspecting images.
    #[serde(rename = "RepoTags")]
    _repo_tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct InspectState {
    #[serde(rename = "Status")]
    pub status: String,

    #[serde(rename = "Running")]
    pub running: bool,

    #[serde(rename = "Error")]
    pub _error: String,

    #[serde(rename = "ExitCode")]
    pub _exit_code: i32,
}

fn command_json<T>(command: &mut Command) -> Result<T>
where
    T: serde::de::DeserializeOwned + std::fmt::Debug,
{
    let output = command.output()?;
    if !output.status.success() {
        if output.stderr.is_empty() {
            bail!("Command failed with no stderr output");
        } else {
            bail!(String::from_utf8_lossy(&output.stderr).to_string());
        }
    } else {
        Ok(serde_json::from_slice(&output.stdout)?)
    }
}

pub(crate) fn find_manager(podman: bool) -> Option<ContainerManager> {
    if !podman {
        debug!("Looking for Docker container engine");

        let manager = ContainerManager::Docker(DockerManager::new());
        if manager.exists() {
            info!("Found Docker container engine");
            if let Ok(version) = manager.version() {
                debug!("Found Docker version {version}");
                return Some(manager);
            }
        } else {
            info!("Docker not found");
        }
    };

    debug!("Looking for Podman container engine");
    let manager = ContainerManager::Podman(PodmanManager::new());
    if manager.exists() {
        info!("Found Podman container engine");
        if let Ok(version) = manager.version() {
            debug!("Found Podman version {version}");
            match semver::Version::parse(&version) {
                Ok(version) => {
                    if version.major < 4 || (version.major == 4 && version.minor < 6) {
                        error!("Podman version must be at least 4.7.0");
                    } else {
                        return Some(manager);
                    }
                }
                Err(_) => {
                    error!("Failed to parse Podman version");
                }
            }
        }
    } else {
        info!("Podman not found");
    }

    None
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum Container {
    Suricata,
    EveBox,
}

impl Container {
    pub(crate) fn volumes(&self) -> Vec<String> {
        match self {
            Container::Suricata => {
                vec![format!("{}:/var/log/suricata", SURICATA_VOLUME_LOG)]
            }
            Container::EveBox => {
                vec![
                    format!("{}:/var/log/suricata", SURICATA_VOLUME_LOG),
                    format!("{}:/var/lib/evebox", EVEBOX_VOLUME_LIB),
                ]
            }
        }
    }
}

/// Given the context and a Container, return an image name, which is
/// either the default or a user configured value.
pub(crate) fn image_name(context: &Context, container: Container) -> String {
    match container {
        Container::Suricata => context
            .config
            .suricata
            .image
            .as_deref()
            .unwrap_or(DEFAULT_SURICATA_IMAGE)
            .to_string(),
        Container::EveBox => context
            .config
            .evebox
            .image
            .as_deref()
            .unwrap_or(DEFAULT_EVEBOX_IMAGE)
            .to_string(),
    }
}

pub(crate) struct SuricataContainer {
    context: Context,
}

impl SuricataContainer {
    pub(crate) fn new(context: Context) -> Self {
        Self { context }
    }

    pub(crate) fn volumes(&self) -> Vec<String> {
        vec![
            format!("{}:/var/log/suricata", SURICATA_VOLUME_LOG),
            format!("{}:/var/lib/suricata", SURICATA_VOLUME_LIB),
            format!("{}:/var/run/suricata", SURICATA_VOLUME_RUN),
        ]
    }

    pub(crate) fn run(&self) -> RunCommandBuilder {
        let mut builder = RunCommandBuilder::new(
            self.context.manager,
            image_name(&self.context, Container::Suricata),
        );
        builder.volumes(&self.volumes());
        builder
    }
}

pub(crate) struct RunCommandBuilder {
    manager: ContainerManager,
    image: String,
    rm: bool,
    it: bool,
    volumes: Vec<String>,
    name: Option<String>,
    args: Vec<String>,
}

impl RunCommandBuilder {
    pub(crate) fn new(manager: ContainerManager, image: impl ToString) -> Self {
        Self {
            manager,
            image: image.to_string(),
            rm: false,
            it: false,
            volumes: vec![],
            name: None,
            args: vec![],
        }
    }

    pub(crate) fn rm(&mut self) -> &mut Self {
        self.rm = true;
        self
    }

    pub(crate) fn it(&mut self) -> &mut Self {
        self.it = true;
        self
    }

    pub(crate) fn _name(&mut self, name: impl ToString) -> &mut Self {
        self.name = Some(name.to_string());
        self
    }

    pub(crate) fn _arg(&mut self, arg: impl ToString) -> &mut Self {
        self.args.push(arg.to_string());
        self
    }

    pub(crate) fn args(&mut self, args: &[impl ToString]) -> &mut Self {
        for arg in args {
            self.args.push(arg.to_string());
        }
        self
    }

    pub(crate) fn volumes(&mut self, volumes: &[impl ToString]) -> &mut Self {
        for volume in volumes {
            self.volumes.push(volume.to_string());
        }
        self
    }

    pub(crate) fn build(&self) -> Command {
        let mut command = self.manager.command();
        command.arg("run");
        if self.it {
            command.arg("-it");
        }
        if self.rm {
            command.arg("--rm");
        }
        if let Some(name) = &self.name {
            command.arg(format!("--name={}", name));
        }
        for volume in &self.volumes {
            command.arg(format!("--volume={}", volume));
        }
        command.arg(&self.image);
        command.args(&self.args);
        command
    }
}
