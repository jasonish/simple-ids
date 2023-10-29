// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{self, Stdio},
    sync::mpsc::Sender,
    thread,
};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use config::Config;
use container::{Container, ContainerManager, SuricataContainer};
use logs::LogArgs;
use tracing::{debug, error, info, warn, Level};

mod actions;
mod config;
mod container;
mod logs;
mod menu;
mod menus;
mod prompt;
mod selfupdate;
mod system;
mod term;

const SURICATA_IMAGE: &str = "docker.io/jasonish/suricata:7.0";
const EVEBOX_IMAGE: &str = "docker.io/jasonish/evebox:master";

const SURICATA_CONTAINER_NAME: &str = "simplensm-suricata";
const EVEBOX_CONTAINER_NAME: &str = "simplensm-evebox";

const SURICATA_VOLUME_LOG: &str = "simplensm-suricata-log";
const SURICATA_VOLUME_LIB: &str = "simplensm-suricata-lib";
const SURICATA_VOLUME_RUN: &str = "simplensm-suricata-run";

const EVEBOX_VOLUME_LIB: &str = "simplensm-evebox-lib";

#[derive(Parser, Debug)]
struct Args {
    /// Use Podman, by default Docker is used if found
    #[arg(long)]
    podman: bool,

    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Start {
        #[arg(long, short)]
        detach: bool,
    },
    Stop,
    Status,
    UpdateRules,
    Update,

    /// View the container logs
    Logs(LogArgs),

    // Commands to jump to specific menus.
    ConfigureMenu,
}

struct Context {
    config: Config,
    manager: ContainerManager,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Mainly for use when developing...
    let _ = std::process::Command::new("stty").args(["sane"]).status();

    let args = Args::parse();

    let log_level = if args.verbose > 0 {
        Level::DEBUG
    } else {
        Level::INFO
    };
    tracing_subscriber::fmt().with_max_level(log_level).init();

    let config = config::Config::new();

    let manager = match container::find_manager() {
        Some(manager) => manager,
        None => {
            error!("No container manager found. Docker or Podman must be available.");
            error!("See https://evebox.org/runtimes/ for more info.");
            std::process::exit(1);
        }
    };
    if manager == container::ContainerManager::Podman && system::getuid() != 0 {
        error!("The Podman container manager requires running as root");
        std::process::exit(1);
    }
    debug!("Found container manager {manager}");

    let mut context = Context { config, manager };

    if let Some(command) = args.command {
        let code = match command {
            Commands::Start { detach } => command_start(&context, detach),
            Commands::Stop => {
                if stop(&context) {
                    0
                } else {
                    1
                }
            }
            Commands::Status => command_status(&context),
            Commands::UpdateRules => {
                if actions::update_rules(&context).is_ok() {
                    0
                } else {
                    1
                }
            }
            Commands::Update => {
                if update(&context) {
                    0
                } else {
                    1
                }
            }
            Commands::ConfigureMenu => {
                menu::configure::main(&mut context);
                0
            }
            Commands::Logs(args) => {
                logs::logs(&context, args);
                0
            }
        };
        std::process::exit(code);
    } else {
        menu_main(context);
    }

    Ok(())
}

fn process_output_handler<R: Read + Sync + Send + 'static>(
    output: R,
    label: &'static str,
    tx: Sender<bool>,
) {
    let reader = BufReader::new(output).lines();
    thread::spawn(move || {
        for line in reader {
            if let Ok(line) = line {
                // Add some coloring to the Suricata output as it
                // doesn't add its own color when writing to a
                // non-interactive terminal.
                let line = if line.starts_with("Info") {
                    line.green().to_string()
                } else if line.starts_with("Error") {
                    line.red().to_string()
                } else if line.starts_with("Notice") {
                    line.magenta().to_string()
                } else if line.starts_with("Warn") {
                    line.yellow().to_string()
                } else {
                    line.to_string()
                };
                let mut stdout = std::io::stdout().lock();
                let _ = writeln!(&mut stdout, "{}: {}", label, line);
                let _ = stdout.flush();
            } else {
                debug!("{}: EOF", label);
                break;
            }
        }
        let _ = tx.send(true);
    });
}

/// Run when "start" is run from the command line.
fn command_start(context: &Context, detach: bool) -> i32 {
    if detach {
        start(context);
        0
    } else {
        start_foreground(context)
    }
}

/// Start SimpleNSM in the foreground.
///
/// Typically not done from the menus but instead the command line.
fn start_foreground(context: &Context) -> i32 {
    context.manager.quiet_rm(SURICATA_CONTAINER_NAME);
    context.manager.quiet_rm(EVEBOX_CONTAINER_NAME);

    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let mut suricata_command = match build_suricata_command(context, false) {
        Ok(command) => command,
        Err(err) => {
            error!("Invalid Suricata configuration: {}", err);
            return 1;
        }
    };

    let mut suricata_process = match suricata_command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(process) => process,
        Err(err) => {
            error!("Failed to spawn Suricata process: {}", err);
            return 1;
        }
    };

    let mut evebox_command = build_evebox_command(context, false);
    let mut evebox_process = match evebox_command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(process) => process,
        Err(err) => {
            error!("Failed to spawn EveBox process: {}", err);
            return 1;
        }
    };

    {
        let tx = tx.clone();
        if let Err(err) = ctrlc::set_handler(move || {
            info!("Received Ctrl-C, stopping containers");
            let _ = tx.send(true);
        }) {
            error!("Failed to setup Ctrl-C handler: {}", err);
        }
    }

    let now = std::time::Instant::now();
    loop {
        if !context.manager.is_running(SURICATA_CONTAINER_NAME) {
            if now.elapsed().as_secs() > 3 {
                error!("Timed out waiting for the Suricata container to start running, not starting log rotation");
                break;
            } else {
                continue;
            }
        }

        if let Err(err) = start_suricata_logrotate(context) {
            error!("Failed to start Suricata log rotation: {err}");
        }
        break;
    }

    if let Some(output) = suricata_process.stdout.take() {
        process_output_handler(output, "suricata", tx.clone());
    }
    if let Some(output) = suricata_process.stderr.take() {
        process_output_handler(output, "suricata", tx.clone());
    }

    if let Some(output) = evebox_process.stdout.take() {
        process_output_handler(output, "evebox", tx.clone());
    }
    if let Some(output) = evebox_process.stderr.take() {
        process_output_handler(output, "evebox", tx.clone());
    }

    let _ = rx.recv();
    let _ = context.manager.stop(SURICATA_CONTAINER_NAME, None);
    let _ = context.manager.stop(EVEBOX_CONTAINER_NAME, Some("SIGINT"));
    let status = suricata_process.wait();
    debug!("Suricata exit status: {:?}", status);
    let status = evebox_process.wait();
    debug!("EveBox exit status: {:?}", status);
    0
}

fn stop(context: &Context) -> bool {
    let mut ok = true;

    if context.manager.exists(SURICATA_CONTAINER_NAME) {
        info!("Stopping {SURICATA_CONTAINER_NAME}");
        if let Err(err) = context.manager.stop(SURICATA_CONTAINER_NAME, None) {
            error!(
                "Failed to stop container {SURICATA_CONTAINER_NAME}: {}",
                err
            );
            ok = false;
        }
        context.manager.quiet_rm(SURICATA_CONTAINER_NAME);
    } else {
        info!("Container {SURICATA_CONTAINER_NAME} is not running");
    }
    if context.manager.exists(EVEBOX_CONTAINER_NAME) {
        info!("Stopping {EVEBOX_CONTAINER_NAME}");
        if let Err(err) = context.manager.stop(EVEBOX_CONTAINER_NAME, Some("SIGINT")) {
            error!("Failed to stop container {EVEBOX_CONTAINER_NAME}: {}", err);
            ok = false;
        }
        context.manager.quiet_rm(EVEBOX_CONTAINER_NAME);
    } else {
        info!("Container {EVEBOX_CONTAINER_NAME} is not running");
    }

    ok
}

fn command_status(context: &Context) -> i32 {
    let mut code = 0;
    match context.manager.state(SURICATA_CONTAINER_NAME) {
        Ok(state) => info!("suricata: {}", state.status),
        Err(err) => {
            warn!("suricata: {}", err);
            code = 1;
        }
    }
    match context.manager.state(EVEBOX_CONTAINER_NAME) {
        Ok(state) => info!("evebox: {}", state.status),
        Err(err) => {
            warn!("evebox: {}", err);
            code = 1;
        }
    }
    code
}

fn guess_evebox_url(context: &Context) -> String {
    let scheme = if context.config.evebox.no_tls {
        "http"
    } else {
        "https"
    };

    if !context.config.evebox.allow_remote {
        format!("{}://127.0.0.1:5636", scheme)
    } else {
        let interfaces = match system::get_interfaces() {
            Ok(interfaces) => interfaces,
            Err(err) => {
                error!("Failed to get system interfaces: {err}");
                return format!("{}://127.0.0.1:5636", scheme);
            }
        };

        // Find the first interface that is up...
        let mut addr: Option<&String> = None;

        for interface in &interfaces {
            // Only consider IPv4 addresses for now.
            if interface.addr4.is_empty() {
                continue;
            }
            if interface.name == "lo" && addr.is_none() {
                addr = interface.addr4.first();
            } else if interface.status == "UP" {
                match addr {
                    Some(previous) => {
                        if previous.starts_with("127") {
                            addr = interface.addr4.first();
                        }
                    }
                    None => {
                        addr = interface.addr4.first();
                    }
                }
            }
        }

        format!(
            "{}://{}:5636",
            scheme,
            addr.unwrap_or(&"127.0.0.1".to_string())
        )
    }
}

fn menu_main(mut context: Context) {
    loop {
        term::clear_title("SimpleNSM: Main Menu");

        let evebox_url = guess_evebox_url(&context);

        let suricata_state = context
            .manager
            .state(SURICATA_CONTAINER_NAME)
            .map(|state| state.status)
            .unwrap_or_else(|_| "not running".to_string());
        let evebox_state = context
            .manager
            .state(EVEBOX_CONTAINER_NAME)
            .map(|state| {
                if state.status == "running" {
                    format!("{} {}", state.status, evebox_url,)
                } else {
                    state.status
                }
            })
            .unwrap_or_else(|_| "not running".to_string());

        let running = context.manager.is_running(SURICATA_CONTAINER_NAME)
            || context.manager.is_running(EVEBOX_CONTAINER_NAME);

        println!(
            "{} Suricata: {} {} EveBox: {}",
            ">>>".cyan(),
            suricata_state,
            ">>>".cyan(),
            evebox_state
        );
        println!();

        let interface = context
            .config
            .suricata
            .interfaces
            .get(0)
            .map(String::from)
            .unwrap_or_default();

        let selections = vec![
            SelectItem::new("refresh", "Refresh Status"),
            if running {
                SelectItem::new("stop", "Stop")
            } else {
                SelectItem::new("start", "Start")
            },
            SelectItem::new("interface", format!("Select Interface [{interface}]")),
            SelectItem::new("update-rules", "Update Rules"),
            SelectItem::new("update", "Update"),
            SelectItem::new("configure", "Configure"),
            SelectItem::new("other", "Other"),
            SelectItem::new("exit", "Exit"),
        ];
        let selections = add_index(&selections);
        let response = inquire::Select::new("Select a menu option", selections)
            .with_page_size(12)
            .prompt();
        match response {
            Ok(selection) => match selection.tag.as_ref() {
                "refresh" => {}
                "start" => {
                    if !start(&context) {
                        prompt::enter();
                    }
                }
                "stop" => {
                    if !stop(&context) {
                        prompt::enter();
                    }
                }
                "interface" => select_interface(&mut context),
                "update" => {
                    update(&context);
                    prompt::enter();
                }
                "other" => menus::other(&context),
                "configure" => menu::configure::main(&mut context),
                "update-rules" => {
                    if let Err(err) = actions::update_rules(&context) {
                        error!("{}", err);
                    }
                    prompt::enter();
                }
                "exit" => break,
                _ => panic!("Unhandled selection: {}", selection.tag),
            },
            Err(_) => break,
        }
    }
}

/// Returns true if everything started successfully, otherwise false
/// is return.
fn start(context: &Context) -> bool {
    let mut ok = true;
    info!("Starting Suricata");
    if let Err(err) = start_suricata_detached(context) {
        error!("Failed to start Suricata: {}", err);
        ok = false;
    }
    info!("Starting EveBox");
    if let Err(err) = start_evebox_detached(context) {
        error!("Failed to start EveBox: {}", err);
        ok = false;
    }
    ok
}

fn build_suricata_command(context: &Context, detached: bool) -> Result<std::process::Command> {
    let interface = match context.config.suricata.interfaces.get(0) {
        Some(interface) => interface,
        None => bail!("no network interface set"),
    };

    let mut args = ArgBuilder::from(&[
        "run",
        "--name",
        SURICATA_CONTAINER_NAME,
        "--net=host",
        "--cap-add=sys_nice",
        "--cap-add=net_admin",
        "--cap-add=net_raw",
    ]);

    if detached {
        args.add("-d");
    }

    for volume in SuricataContainer::new(context.manager).volumes() {
        args.add(format!("--volume={}", volume));
    }

    args.extend(&[SURICATA_IMAGE, "-v", "-i", interface]);

    let mut command = context.manager.command();
    command.args(&args.args);
    Ok(command)
}

fn start_suricata_detached(context: &Context) -> Result<()> {
    context.manager.quiet_rm(SURICATA_CONTAINER_NAME);
    let mut command = build_suricata_command(context, true)?;
    let output = command.output()?;
    if !output.status.success() {
        bail!(String::from_utf8_lossy(&output.stderr).to_string());
    }

    if let Err(err) = start_suricata_logrotate(context) {
        error!("{}", err);
    }
    Ok(())
}

fn start_suricata_logrotate(context: &Context) -> Result<()> {
    info!("Starting Suricata log rotation");
    match context
        .manager
        .command()
        .args([
            "exec",
            "-d",
            SURICATA_CONTAINER_NAME,
            "bash",
            "-c",
            "while true; do logrotate -v /etc/logrotate.d/suricata > /tmp/last_logrotate 2>&1; sleep 600; done",
        ])
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                bail!(String::from_utf8_lossy(&output.stderr).to_string());
            }
        }
        Err(err) => bail!("Failed to initialize log rotation: {err}"),
    }
    Ok(())
}

fn build_evebox_command(context: &Context, daemon: bool) -> process::Command {
    let mut args = ArgBuilder::from(&[
        "run",
        "--name",
        EVEBOX_CONTAINER_NAME,
        // "--restart=unless-stopped",
    ]);
    if context.config.evebox.allow_remote {
        args.add("--publish=5636:5636");
    } else {
        args.add("--publish=127.0.0.1:5636:5636");
    }
    if daemon {
        args.add("-d");
    }

    for volume in Container::EveBox.volumes() {
        args.add(format!("--volume={}", volume));
    }

    args.extend(&[EVEBOX_IMAGE, "evebox", "server"]);

    if context.config.evebox.no_tls {
        args.add("--no-tls");
    }

    if context.config.evebox.no_auth {
        args.add("--no-auth");
    }

    args.extend(&["--host=[::0]", "--sqlite", "/var/log/suricata/eve.json"]);
    let mut command = context.manager.command();
    command.args(&args.args);
    command
}

fn start_evebox_detached(context: &Context) -> Result<()> {
    actions::start_evebox(context)
}

fn select_interface(context: &mut Context) {
    let interfaces = system::get_interfaces().unwrap();
    let current_if = context.config.suricata.interfaces.get(0);
    let index = interfaces
        .iter()
        .position(|interface| Some(&interface.name) == current_if)
        .unwrap_or(0);
    let selections: Vec<SelectItem> = interfaces
        .iter()
        .enumerate()
        .map(|(i, ifname)| {
            let address = ifname
                .addr4
                .first()
                .map(|s| format!("-- {}", s.green().italic()))
                .unwrap_or("".to_string());
            SelectItem::new(
                ifname.name.to_string(),
                format!("{}) {} {}", i + 1, ifname.name, address),
            )
        })
        .collect();
    match inquire::Select::new("Select interface", selections)
        .with_starting_cursor(index)
        .with_page_size(12)
        .prompt()
    {
        Err(_) => {}
        Ok(selection) => {
            context.config.suricata.interfaces = vec![selection.tag.to_string()];
            let _ = context.config.save();
        }
    }
}

fn update(context: &Context) -> bool {
    let mut ok = true;
    for image in [SURICATA_IMAGE, EVEBOX_IMAGE] {
        if let Err(err) = context.manager.pull(image) {
            error!("Failed to pull {image}: {err}");
            ok = false;
        }
    }
    if let Err(err) = selfupdate::self_update() {
        error!("Failed to update SimpleNSM: {err}");
        ok = false;
    }
    ok
}

#[derive(Debug, Clone)]
struct SelectItem {
    tag: String,
    label: String,
}

impl SelectItem {
    fn new(tag: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            label: label.into(),
        }
    }
}

impl std::fmt::Display for SelectItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

fn add_index(selections: &[SelectItem]) -> Vec<SelectItem> {
    selections
        .iter()
        .enumerate()
        .map(|(i, e)| SelectItem::new(e.tag.to_string(), format!("{}. {}", i + 1, e.label)))
        .collect()
}

/// Utility for building arguments for commands.
#[derive(Debug, Default)]
struct ArgBuilder {
    args: Vec<String>,
}

impl ArgBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn from<S: AsRef<str>>(args: &[S]) -> Self {
        let mut builder = Self::default();
        builder.extend(args);
        builder
    }

    fn add(&mut self, arg: impl Into<String>) -> &mut Self {
        self.args.push(arg.into());
        self
    }

    fn extend<S: AsRef<str>>(&mut self, args: &[S]) -> &mut Self {
        for arg in args {
            self.args.push(arg.as_ref().to_string());
        }
        self
    }
}
