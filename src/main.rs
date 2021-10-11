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

#![allow(clippy::needless_return)]

use std::io::Write;
use std::process::Command;

use anyhow::Result;
use clap::Clap;

use crate::config::Config;
use crate::container::ContainerRuntime;
use crate::term::dummy_prompt;

use tracing::{error, info};

#[macro_use]
mod term;
mod config;
mod container;
mod docker;
mod ffi;
mod podman;
mod update;

const DEFAULT_SURICATA_IMAGE: &str = "docker.io/jasonish/suricata:latest";
const SURICATA_CONTAINER_NAME: &str = "easy-suricata";

const DEFAULT_EVEBOX_IMAGE: &str = "docker.io/jasonish/evebox:master";
const EVEBOX_CONTAINER_NAME: &str = "easy-evebox";

const TITLE_PREFIX: &str = "Easy - Suricata/EveBox";

#[derive(Clap, Debug, Clone)]
#[clap(name = "easy-suricata", setting = clap::AppSettings::ColoredHelp)]
struct Opts {
    /// Use Podman instead of Docker
    #[clap(long)]
    podman: bool,

    /// Enable debug output
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,

    #[clap(subcommand)]
    command: Option<SubCommand>,
}

#[derive(Clap, Debug, Clone)]
enum SubCommand {
    UpdateRules,
}

enum Volume {
    Log,
    Run,
    Lib,
    Etc,
    EveBoxData,
}

struct Context {
    config: Config,
    runtime: ContainerRuntime,
    interactive: bool,
}

impl Context {
    fn get_volume_config(&self, volume: Volume) -> String {
        let source = self.get_volume_source(&volume);
        match volume {
            Volume::Etc => format!("--volume={}:/etc/suricata", source),
            Volume::Run => format!("--volume={}:/run/suricata", source),
            Volume::Lib => format!("--volume={}:/var/lib/suricata", source),
            Volume::EveBoxData => format!("--volume={}:/data", source),
            Volume::Log => format!("--volume={}:/var/log/suricata", source),
        }
    }

    fn get_volume_source(&self, volume: &Volume) -> String {
        let data_directory = if let Some(dir) = &self.config.data_directory {
            format!("{}/", dir)
        } else {
            "".to_string()
        };
        let source = match volume {
            Volume::Etc => format!("{}--etc", SURICATA_CONTAINER_NAME),
            Volume::Run => format!("{}--run", SURICATA_CONTAINER_NAME),
            Volume::Lib => format!("{}--lib", SURICATA_CONTAINER_NAME),
            Volume::Log => format!("{}{}--log", &data_directory, SURICATA_CONTAINER_NAME),
            Volume::EveBoxData => format!("{}{}--data", &data_directory, EVEBOX_CONTAINER_NAME),
        };

        // If source is a real path, try to make sure it exists (required for Podman).
        if source.starts_with('/') {
            if let Err(err) = ensure_exists(&source) {
                match self.runtime {
                    ContainerRuntime::Docker => {}
                    ContainerRuntime::Podman => {
                        tracing::error!("Failed to create directory {}: {}", source, err);
                    }
                }
            }
        }

        source
    }
}

fn init_logging(opts: &Opts) {
    let level = if opts.verbose > 1 {
        tracing::Level::TRACE
    } else if opts.verbose > 0 {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(format!(
            "{},hyper=off,warp=off",
            &level.to_string().to_lowercase()
        ))
        .init();
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    init_logging(&opts);

    let config = config::Config::new();

    let runtime = match container::find_runtime(opts.podman) {
        None => {
            tracing::error!("No container runtime found. Docker or Podman must be available.");
            tracing::error!("See https://evebox.org/runtimes/ for more info.");
            std::process::exit(1);
        }
        Some(runtime) => {
            tracing::info!("Found container runtime: {}", runtime.program_name());
            runtime
        }
    };

    // Podman requires root...
    if runtime == ContainerRuntime::Podman && ffi::getuid() != 0 {
        tracing::error!("The Podman container runtime requires running as root");
    }

    let interactive = opts.command.is_none();
    let mut context = Context {
        config,
        runtime,
        interactive,
    };

    match &opts.command {
        None => main_menu(&mut context)?,
        Some(sub) => match sub {
            SubCommand::UpdateRules => update_rules(&context)?,
        },
    }
    Ok(())
}

fn version() -> &'static str {
    std::env!("CARGO_PKG_VERSION")
}

struct DebugMenu<'a> {
    context: &'a Context,
}

impl<'a> DebugMenu<'a> {
    fn new(context: &'a Context) -> Self {
        Self { context }
    }

    fn run(&self) -> Result<()> {
        loop {
            println!("1. View last 20 lines of Suricata log");
            println!("2. View last 20 lines of EveBox log");
            println!("3. Tail Suricata log");
            println!("4. Tail EveBox log");
            println!("x. Exit");
            println!();
            print!("Select menu option: ");
            match term::read_line().as_ref() {
                "1" => self.last_n(SURICATA_CONTAINER_NAME),
                "2" => self.last_n(EVEBOX_CONTAINER_NAME),
                "3" => self.tail(SURICATA_CONTAINER_NAME),
                "4" => self.tail(EVEBOX_CONTAINER_NAME),
                "x" => break,
                _ => {}
            }
        }
        Ok(())
    }

    fn last_n(&self, name: &str) {
        let _ = self
            .context
            .runtime
            .exec_status(&["logs", "--tail=20", name]);
        term::prompt_for_enter();
    }

    fn tail(&self, name: &str) {
        dummy_prompt("Hit CTRL-C to exit. This will return you to the shell, not this program: ");
        let _ = self
            .context
            .runtime
            .exec_status(&["logs", "--tail=20", "--follow", name]);
    }
}

struct ConfigureMenu<'a> {
    context: &'a mut Context,
}

impl<'a> ConfigureMenu<'a> {
    fn new(context: &'a mut Context) -> Self {
        Self { context }
    }

    fn run(&mut self) -> Result<()> {
        loop {
            let interface = self
                .context
                .config
                .interface
                .as_ref()
                .unwrap_or(&"none".to_string())
                .to_string();

            println!("1. Set Interface [{}]", interface);
            if self.context.config.evebox.enabled {
                println!("2. Disable EveBox");
            } else {
                println!("2. Enable EveBox");
            }
            if self.context.config.evebox.allow_external {
                println!("3. EveBox: Disable External Access (currently enabled)");
            } else {
                println!("3. EveBox: Enable External Access (currently disabled)");
            };
            println!(
                "4. BPF Filter: {}",
                if let Some(bpf) = &self.context.config.bpf_filter {
                    bpf
                } else {
                    "(not set)"
                }
            );
            println!("5. Start on Boot: {}", self.context.config.start_on_boot);
            println!(
                "6. Data Directory: {}",
                self.context
                    .config
                    .data_directory
                    .as_ref()
                    .unwrap_or(&"(none)".to_string())
            );
            println!("x. Exit");
            println!();

            print!("Select menu option: ");
            match term::read_line().as_ref() {
                "1" => set_monitor_interface_menu(&mut self.context)?,
                "2" => self.toggle_evebox()?,
                "3" => self.toggle_evebox_external_access()?,
                "4" => self.set_bpf_filter()?,
                "5" => self.toggle_start_on_boot()?,
                "6" => self.set_data_directory()?,
                "x" => break,
                _ => {}
            }
        }
        Ok(())
    }

    fn toggle_start_on_boot(&mut self) -> Result<()> {
        if self.context.runtime == ContainerRuntime::Podman {
            term::print_err("Start on boot is not supported with Podman yet.");
            term::prompt_for_enter();
            return Ok(());
        }
        self.context.config.start_on_boot = !self.context.config.start_on_boot;
        self.context.config.save(None)?;
        Ok(())
    }

    fn set_bpf_filter(&mut self) -> Result<()> {
        print!("Enter new BPF filter: ");
        let filter = term::read_line();
        if filter.is_empty() {
            self.context.config.bpf_filter = None;
        } else {
            self.context.config.bpf_filter = Some(filter);
        }
        self.context.config.save(None)?;
        Ok(())
    }

    fn set_data_directory(&mut self) -> Result<()> {
        if ffi::getuid() != 0 {
            println!();
            println!("NOTE: It is possible to use a directory that may not be writable");
            println!("      by the user running easy. If so, create the directory");
            println!("      then come back and configure it here.");
        }
        print!("Enter data directory: ");
        let directory = term::read_line();
        if directory.is_empty() {
            self.context.config.data_directory = None;
        } else {
            if let Err(err) = ensure_exists(&directory) {
                tracing::error!("Failed to set data directory: {}: {}", &directory, err);
                return Ok(());
            }
            match std::fs::canonicalize(&directory) {
                Ok(path) => {
                    self.context.config.data_directory = Some(path.to_str().unwrap().to_string());
                }
                Err(err) => {
                    tracing::error!("Bad directory: {}", err);
                }
            }
        }
        self.context.config.save(None)?;
        Ok(())
    }

    fn toggle_evebox_external_access(&mut self) -> Result<()> {
        self.context.config.evebox.allow_external = !self.context.config.evebox.allow_external;
        self.context.config.save(None)?;
        if self
            .context
            .runtime
            .container_running(EVEBOX_CONTAINER_NAME)
        {
            tracing::info!("Restarting EveBox...");
            if let Err(err) = stop_evebox(self.context) {
                tracing::error!("Failed to stop EveBox: {}", err);
            }
            if let Err(err) = start_evebox(self.context) {
                tracing::error!("Failed to start EveBox: {}", err);
            }
        }
        Ok(())
    }

    fn toggle_evebox(&mut self) -> Result<()> {
        if self.context.config.evebox.enabled {
            self.context.config.evebox.enabled = false;
        } else {
            println!();
            println!("WARNING: EveBox runs without authentication at this time.");
            println!();
            print!("Are you sure you want to enable Evebox [N/y]? ");
            match term::read_line().to_lowercase().as_ref() {
                "y" | "yes" => {
                    self.context.config.evebox.enabled = true;
                }
                _ => return Ok(()),
            }
        }

        // If EveBox is now disabled, stop it. But if is now enabled, start it if Suricata is
        // currently running.
        if self.context.config.evebox.enabled {
            if self
                .context
                .runtime
                .container_running(SURICATA_CONTAINER_NAME)
                && !self
                    .context
                    .runtime
                    .container_running(EVEBOX_CONTAINER_NAME)
            {
                let _ = start_evebox(self.context);
            }
        } else if self
            .context
            .runtime
            .container_running(EVEBOX_CONTAINER_NAME)
        {
            let _ = stop_evebox(self.context);
        }
        self.context.config.save(None)?;
        Ok(())
    }
}

fn ensure_exists(path: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if !std::path::Path::new(&path).exists() {
        std::fs::create_dir_all(&path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o2750))?;
    }
    Ok(())
}

fn main_menu(context: &mut Context) -> Result<()> {
    let title = format!("{} - Main Menu (v: {})", TITLE_PREFIX, version());

    loop {
        println!("{}\n", &title);

        // Some errors.
        if !context.runtime.image_exists(DEFAULT_SURICATA_IMAGE) {
            term::print_err("Suricata container image does not exist. Run Update.");
        }
        if context.config.interface.is_none() {
            term::print_err("No interface selected, choose Configure.");
        }

        // Some status...
        if context.runtime.container_running(SURICATA_CONTAINER_NAME) {
            term::print_status("Suricata", "running");
        } else {
            term::print_status("Suricata", "not running");
        }
        if let Some(output) = context.runtime.last_log_line(SURICATA_CONTAINER_NAME) {
            term::print_status("Suricata", &output);
        }

        if !context.config.evebox.enabled {
            term::print_status("EveBox", "not enabled");
        } else if context.runtime.container_running(EVEBOX_CONTAINER_NAME) {
            term::print_status("EveBox", "running");
        } else {
            term::print_status("EveBox", "not running");
        }

        println!();

        println!("1. Start");
        println!("2. Stop");
        println!("3. Restart");
        println!("4. Update Rules");
        println!("5. Shell");
        println!("6. Force Log Rotation");
        println!("7. Update");
        println!("8. Configure");
        println!("9. Debug");
        println!("x. Exit");
        println!();
        print!("Select menu option: ");
        match term::read_line().as_ref() {
            "1" => start(context)?,
            "2" => stop(context)?,
            "3" => restart(context)?,
            "4" => update_rules(context)?,
            "5" => shell(context)?,
            "6" => rotate_logs(context)?,
            "7" => update(context)?,
            "8" => ConfigureMenu::new(context).run()?,
            "9" => DebugMenu::new(context).run()?,
            "x" => break,
            _ => {}
        }
    }
    Ok(())
}

fn shell(context: &Context) -> Result<()> {
    let ps1 = r#"PS1=\[\033[1;36m\]easy-suricata \[\033[1;34m\]\w\[\033[0;35m\] \[\033[1;36m\]# \[\033[0m\]"#;
    let mut args = vec![context.runtime.program_name()];
    args.extend_from_slice(&[
        "exec",
        "-it",
        "-e",
        ps1,
        SURICATA_CONTAINER_NAME,
        "/bin/bash",
    ]);
    match Command::new(&args[0]).args(&args[1..]).status() {
        Ok(status) => {
            if !status.success() {
                term::prompt_for_enter();
            }
        }
        Err(err) => {
            eprintln!("ERROR: Failed to execute {}: {}", &args[0], err);
            term::prompt_for_enter();
        }
    }
    Ok(())
}

fn restart(context: &Context) -> Result<()> {
    let _ = stop(context);
    let _ = start(context);
    Ok(())
}

fn start_evebox(context: &Context) -> Result<()> {
    let mut args = vec!["run", "-d", "--name", EVEBOX_CONTAINER_NAME];

    if context.config.evebox.allow_external {
        args.push("--publish=0.0.0.0:5636:5636");
    } else {
        args.push("--publish=127.0.0.1:5636:5636");
    }

    if context.runtime.requires_privilege() {
        args.push("--privileged");
    }

    // If we have /etc/localtime, provide it as a read-only volume.
    if std::path::Path::new("/etc/localtime").exists() {
        args.push("--volume=/etc/localtime:/etc/localtime:ro");
    }

    // The Suricata log volume.
    let log_vol = context.get_volume_config(Volume::Log);
    args.push(&log_vol);

    // EveBox data directory.
    let data_vol = context.get_volume_config(Volume::EveBoxData);
    args.push(&data_vol);

    // Restart policy.
    if context.runtime == ContainerRuntime::Docker && context.config.start_on_boot {
        args.push("--restart=unless-stopped");
    }

    args.push(DEFAULT_EVEBOX_IMAGE);

    args.extend_from_slice(&[
        "evebox",
        "-D",
        "/data",
        "server",
        "--input",
        "/var/log/suricata/eve.json",
        "--datastore",
        "sqlite",
    ]);
    tracing::info!("Starting EveBox");
    tracing::debug!(
        "Executing: {} {}",
        context.runtime.program_name(),
        args.join(" ")
    );
    if let Err(err) = context.runtime.exec_output(&args) {
        tracing::error!("Failed to start EveBox: {}", err);
    }
    Ok(())
}

fn start_suricata(context: &Context) -> Result<()> {
    if context.config.interface.is_none() {
        term::dummy_prompt("Error: Interface is not set, hit enter to continue.");
        return Ok(());
    }

    let mut args = vec![context.runtime.program_name()];
    args.push("run");
    args.push("-d");
    args.push("--net=host");
    args.extend_from_slice(&["--name", SURICATA_CONTAINER_NAME]);

    args.push("--cap-add=sys_nice");
    args.push("--cap-add=net_admin");
    args.push("--cap-add=net_raw");
    if context.runtime.requires_privilege() {
        args.push("--privileged");
    }

    // Volumes.

    // If we have /etc/localtime, provide it as a read-only volume.
    if std::path::Path::new("/etc/localtime").exists() {
        args.push("--volume=/etc/localtime:/etc/localtime:ro");
    }

    let etc_vol = context.get_volume_config(Volume::Etc);
    args.push(&etc_vol);

    // A "run" volume for the command socket, but not exposed to the host fs.
    let run_vol = context.get_volume_config(Volume::Run);
    args.push(&run_vol);

    let log_vol = context.get_volume_config(Volume::Log);
    args.push(&log_vol);

    let lib_vol = context.get_volume_config(Volume::Lib);
    args.push(&lib_vol);

    let disable_vol;
    if let Ok(path) = std::fs::canonicalize("./disable.conf") {
        info!("Found {}", path.display());
        disable_vol = format!("--volume={}:/etc/suricata/disable.conf", path.display());
        args.push(&disable_vol);
    }

    let enable_vol;
    if let Ok(path) = std::fs::canonicalize("./enable.conf") {
        info!("Found {}", path.display());
        enable_vol = format!("--volume={}:/etc/suricata/enable.conf", path.display());
        args.push(&enable_vol);
    }

    // Restart policy.
    if context.runtime == ContainerRuntime::Docker && context.config.start_on_boot {
        args.push("--restart=unless-stopped");
    }

    args.push(DEFAULT_SURICATA_IMAGE);
    args.extend_from_slice(&["-k", "none"]);
    args.extend_from_slice(&["-i", context.config.interface.as_ref().unwrap()]);

    if let Some(bpf) = &context.config.bpf_filter {
        if !bpf.is_empty() {
            args.push(bpf);
        }
    }

    tracing::info!("Starting Suricata");
    tracing::debug!(
        "Executing: {} {}",
        context.runtime.program_name(),
        args.join(" ")
    );
    if let Err(err) = context.runtime.exec_output(&args[1..]) {
        tracing::error!("Failed to start Suricata: {}", err);
    } else {
        // This is a bit of a hack to enable logrotate, exec crond inside the container.
        let args = &["exec", SURICATA_CONTAINER_NAME, "crond"];
        if let Err(err) = context.runtime.exec_status(args) {
            tracing::error!(
                "Failed to to start cron/logrotate in Suricata container: {}",
                err
            );
        }
    }

    Ok(())
}

fn start(context: &Context) -> Result<()> {
    start_suricata(context)?;
    if context.config.evebox.enabled {
        start_evebox(context)?;
    }
    Ok(())
}

fn update_rules(context: &Context) -> Result<()> {
    let args = &["exec", "-it", SURICATA_CONTAINER_NAME, "suricata-update"];
    if let Err(_) = context.runtime.exec_status(args) {
        error!("An error occurred while trying to update the rules.");
    }
    if context.interactive {
        term::prompt_for_enter();
    }
    Ok(())
}

fn rotate_logs(context: &Context) -> Result<()> {
    let args = vec![
        "exec",
        "-it",
        SURICATA_CONTAINER_NAME,
        "logrotate",
        "-vf",
        "/etc/logrotate.conf",
    ];
    let _ = context.runtime.exec_status(&args);
    if context.interactive {
        term::prompt_for_enter();
    }
    Ok(())
}

fn update(context: &Context) -> Result<()> {
    let images = &[DEFAULT_SURICATA_IMAGE, DEFAULT_EVEBOX_IMAGE];
    for image in images {
        let args = vec![context.runtime.program_name(), "pull", image];
        let _ = Command::new(&args[0]).args(&args[1..]).status();
    }
    update::self_update()?;
    term::dummy_prompt("Press ENTER to continue:");
    Ok(())
}

fn stop_container(context: &Context, name: &str) -> Result<()> {
    let args = vec!["rm", "-f", name];
    context.runtime.exec_output(&args)?;
    Ok(())
}

fn stop_suricata(context: &Context) -> Result<()> {
    tracing::info!("Stopping Suricata");
    if let Err(err) = stop_container(context, SURICATA_CONTAINER_NAME) {
        interactive_error(context, &format!("Failed to stop Suricata: {}", err));
    }
    Ok(())
}

fn interactive_error(context: &Context, msg: &str) {
    tracing::error!("{}", msg);
    if context.interactive {
        term::prompt_for_enter();
    }
}

fn stop_evebox(context: &Context) -> Result<()> {
    tracing::info!("Stopping EveBox");
    if let Err(err) = stop_container(context, EVEBOX_CONTAINER_NAME) {
        interactive_error(context, &format!("Failed to stop Evebox: {}", err));
    }
    Ok(())
}

fn stop(context: &Context) -> Result<()> {
    if context.config.evebox.enabled {
        stop_evebox(context)?;
    }
    stop_suricata(context)?;
    Ok(())
}

fn set_monitor_interface_menu(context: &mut Context) -> Result<()> {
    let ifnames = get_interface_names()?;
    loop {
        for (i, interface) in ifnames.iter().enumerate() {
            println!("{}. {}", i + 1, interface);
        }
        println!();
        print!("Select interface from above menu: ");
        if let Ok(n) = term::read_line().parse::<usize>() {
            if let Some(ifname) = ifnames.get(n - 1) {
                context.config.interface = Some(ifname.to_string());
                break;
            }
        }
    }
    context.config.save(None)?;
    Ok(())
}

/// Return a vector of found network interface names.
fn get_interface_names() -> Result<Vec<String>> {
    let mut ifnames = Vec::new();
    let re = regex::Regex::new(r"^\d+:\s+([^\s]+)")?;
    let output = Command::new("ip").arg("link").arg("show").output()?;
    let out = String::from_utf8(output.stdout)?;
    for line in out.split('\n') {
        for cap in re.captures_iter(line) {
            let ifname = if cap[1].ends_with(':') {
                &cap[1][0..&cap[1].len() - 1]
            } else {
                &cap[1]
            };
            ifnames.push(ifname.to_string());
        }
    }
    Ok(ifnames)
}
