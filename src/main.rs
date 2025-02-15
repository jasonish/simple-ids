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
use container::{Container, SuricataContainer};
use logs::LogArgs;
use tracing::{debug, error, info, Level};

use crate::context::Context;

mod actions;
mod config;
mod container;
mod context;
mod logs;
mod menu;
mod menus;
mod prelude;
mod ruleindex;
mod selfupdate;
mod term;

const SURICATA_CONTAINER_NAME: &str = "simple-ids-suricata";
const EVEBOX_CONTAINER_NAME: &str = "simple-ids-evebox";

const SURICATA_VOLUME_LOG: &str = "simple-ids-suricata-log";
const SURICATA_VOLUME_LIB: &str = "simple-ids-suricata-lib";
const SURICATA_VOLUME_RUN: &str = "simple-ids-suricata-run";

const EVEBOX_VOLUME_LIB: &str = "simple-ids-evebox-lib";

fn get_clap_style() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .header(clap::builder::styling::AnsiColor::Yellow.on_default())
        .usage(clap::builder::styling::AnsiColor::Green.on_default())
        .literal(clap::builder::styling::AnsiColor::Green.on_default())
        .placeholder(clap::builder::styling::AnsiColor::Green.on_default())
}

#[derive(Parser, Debug)]
#[command(styles=get_clap_style())]
struct Args {
    /// Use Podman, by default Docker is used if found
    #[arg(long)]
    podman: bool,

    #[arg(long)]
    no_root: bool,

    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(long, help = "Don't apply Suricata fix-ups")]
    no_fixups: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Start {
        /// Run in the foreground, mainly for debugging
        #[arg(long, short)]
        debug: bool,
    },
    Stop,
    Restart,
    Status,
    UpdateRules,
    Update,

    /// View the container logs
    Logs(LogArgs),

    // Commands to jump to specific menus.
    ConfigureMenu,

    Menu {
        menu: String,
    },

    /// Remove containers and data.
    Remove,
}

fn is_interactive(command: &Option<Commands>) -> bool {
    match command {
        Some(command) => match command {
            Commands::Start { debug: _ } => false,
            Commands::Stop => false,
            Commands::Restart => false,
            Commands::Status => false,
            Commands::UpdateRules => false,
            Commands::Update => false,
            Commands::Logs(_) => false,
            Commands::ConfigureMenu => true,
            Commands::Menu { menu: _ } => true,
            Commands::Remove => false,
        },
        None => true,
    }
}

fn confirm(msg: &str) -> bool {
    inquire::Confirm::new(msg).prompt().unwrap_or(false)
}

fn wizard(context: &mut Context) {
    if context.config.suricata.interfaces.is_empty()
        && confirm("No network interface configured, configure now?")
    {
        select_interface(context);
    }
}

fn main() -> Result<()> {
    // Mainly for use when developing...
    let _ = std::process::Command::new("stty").args(["sane"]).status();

    let args = Args::parse();
    let is_interactive = is_interactive(&args.command);

    let log_level = if args.verbose > 0 {
        Level::DEBUG
    } else {
        Level::INFO
    };

    if is_interactive {
        tracing_subscriber::fmt()
            .with_max_level(log_level)
            .without_time()
            .with_target(false)
            .init();
    } else {
        tracing_subscriber::fmt().with_max_level(log_level).init();
    }

    let config = config::Config::new();

    let manager = match container::find_manager(args.podman) {
        Some(manager) => manager,
        None => {
            error!("No container manager found. Docker or Podman must be available.");
            error!("See https://evebox.org/runtimes/ for more info.");
            std::process::exit(1);
        }
    };
    if manager.is_podman() && evectl::system::getuid() != 0 && !args.no_root {
        error!("The Podman container manager requires running as root");
        std::process::exit(1);
    }
    info!("Found container manager {manager}");

    let mut context = Context::new(config, manager, args.no_fixups);

    let prompt_for_update = {
        if let Some(Commands::Remove) = args.command {
            false
        } else {
            let mut not_found = false;
            if !manager.has_image(&context.suricata_image) {
                info!("Suricata image {} not found", &context.suricata_image);
                not_found = true;
            }
            if !manager.has_image(&context.evebox_image) {
                info!("EveBox image {} not found", &context.evebox_image);
                not_found = true
            }
            not_found
        }
    };

    if prompt_for_update {
        if let Ok(true) =
            inquire::Confirm::new("Required container images not found, download now?")
                .with_default(true)
                .prompt()
        {
            if !update(&context) {
                error!("Failed to downloading container images");
                evectl::prompt::enter();
            }
        }
    }

    if let Some(command) = args.command {
        let code = match command {
            Commands::Start { debug: detach } => command_start(&context, detach),
            Commands::Stop => {
                if stop(&context) {
                    0
                } else {
                    1
                }
            }
            Commands::Restart => {
                stop(&context);
                command_start(&context, true)
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
                menu::configure::main(&mut context)?;
                0
            }
            Commands::Logs(args) => {
                logs::logs(&context, args);
                0
            }
            Commands::Menu { menu } => match menu.as_str() {
                "configure.advanced" => {
                    menu::advanced::advanced_menu(&mut context);
                    0
                }
                _ => panic!("Unhandled menu: {}", menu),
            },
            Commands::Remove => {
                remove(&context);
                0
            }
        };
        std::process::exit(code);
    } else {
        menu_main(context)?;
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
fn command_start(context: &Context, debug: bool) -> i32 {
    if debug {
        start_foreground(context)
    } else {
        start(context);
        0
    }
}

/// Start Simple-IDS in the foreground.
///
/// Typically not done from the menus but instead the command line.
fn start_foreground(context: &Context) -> i32 {
    context.manager.quiet_rm(SURICATA_CONTAINER_NAME);
    context.manager.quiet_rm(EVEBOX_CONTAINER_NAME);

    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let mut suricata_command = match build_suricata_command(context, false, true) {
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

    if context.manager.container_exists(SURICATA_CONTAINER_NAME) {
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
    if context.manager.container_exists(EVEBOX_CONTAINER_NAME) {
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
            let err = format!("{}", err);
            error!("suricata: {}", err.trim_end());
            code = 1;
        }
    }
    match context.manager.state(EVEBOX_CONTAINER_NAME) {
        Ok(state) => info!("evebox: {}", state.status),
        Err(err) => {
            let err = format!("{}", err);
            error!("evebox: {}", err.trim_end());
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
        let interfaces = match evectl::system::get_interfaces() {
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

fn menu_main(mut context: Context) -> Result<()> {
    let mut first = true;
    loop {
        term::title("Simple-IDS: Main Menu");

        if first {
            first = false;
            wizard(&mut context);
        }

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
            .first()
            .map(String::from)
            .unwrap_or_default();

        let mut selections = evectl::prompt::Selections::with_index();
        selections.push("refresh", "Refresh Status");
        if running {
            selections.push("restart", "Restart");
            selections.push("stop", "Stop");
        } else {
            selections.push("start", "Start");
        }
        selections.push("interface", format!("Select Interface [{interface}]"));
        selections.push("update-rules", "Update Rules");
        selections.push("update", "Update");
        selections.push("configure", "Configure");
        selections.push("other", "Other");
        selections.push("exit", "Exit");

        let response = inquire::Select::new("Select a menu option", selections.to_vec())
            .with_page_size(12)
            .prompt();
        match response {
            Ok(selection) => match selection.tag {
                "refresh" => {}
                "start" => {
                    if !start(&context) {
                        evectl::prompt::enter();
                    }
                }
                "stop" => {
                    if !stop(&context) {
                        evectl::prompt::enter();
                    }
                }
                "restart" => {
                    stop(&context);
                    if !start(&context) {
                        evectl::prompt::enter();
                    }
                }
                "interface" => select_interface(&mut context),
                "update" => {
                    update(&context);
                    evectl::prompt::enter();
                }
                "other" => menus::other(&context),
                "configure" => menu::configure::main(&mut context)?,
                "update-rules" => {
                    if let Err(err) = actions::update_rules(&context) {
                        error!("{}", err);
                    }
                    evectl::prompt::enter();
                }
                "exit" => break,
                _ => panic!("Unhandled selection: {}", selection.tag),
            },
            Err(_) => break,
        }
    }

    Ok(())
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

fn build_suricata_command(context: &Context, detached: bool, stubs: bool) -> Result<std::process::Command> {
    let interface = match context.config.suricata.interfaces.first() {
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

    if !context.no_fixups && stubs {
        // Write out af-packet stub for fixed af-packet.
        let path = std::env::current_dir()?.join("af-packet.yaml");
        evectl::configs::write_af_packet_stub(&path)?;
        args.add(format!(
            "--volume={}:/config/af-packet.yaml",
            path.display()
        ));
    }

    for volume in SuricataContainer::new(context.clone()).volumes() {
        args.add(format!("--volume={}", volume));
    }

    args.add(context.image_name(Container::Suricata));
    args.extend(&["-v", "-i", interface]);

    if !context.no_fixups || stubs {
        args.add("--include");
        args.add("/config/af-packet.yaml");
    }

    if let Some(bpf) = &context.config.suricata.bpf {
        args.add(bpf);
    }

    let mut command = context.manager.command();
    command.args(&args.args);
    Ok(command)
}

fn suricata_dump_config(context: &Context) -> Result<Vec<String>> {
    context.manager.quiet_rm(SURICATA_CONTAINER_NAME);
    let mut command = build_suricata_command(context, false, false)?;
    command.arg("--dump-config");
    let output = command.output()?;
    if output.status.success() {
        let stdout = std::str::from_utf8(&output.stdout)?;
        let lines: Vec<String> = stdout.lines().map(|s| s.to_string()).collect();
        Ok(lines)
    } else {
        bail!("Failed to run --dump-config for Suricata")
    }
}

fn start_suricata_detached(context: &Context) -> Result<()> {
    let config = suricata_dump_config(context)?;
    let mut set_args: Vec<String> = vec![
        "app-layer.protocols.tls.ja4-fingerprints=true".to_string(),
        "app-layer.protocols.quic.ja4-fingerprints=true".to_string(),
    ];
    let patterns = &[
        regex::Regex::new(r"(outputs\.\d+\.eve-log\.types\.\d+\.tls)\s")?,
        regex::Regex::new(r"(outputs\.\d+\.eve-log\.types\.\d+\.quic)\s")?,
    ];
    for line in &config {
        for r in patterns {
            if let Some(c) = r.captures(line) {
                set_args.push(format!("{}.ja4=true", &c[1]));
            }
        }
    }

    context.manager.quiet_rm(SURICATA_CONTAINER_NAME);
    let mut command = build_suricata_command(context, true, true)?;
    for s in &set_args {
        command.arg("--set");
        command.arg(s);
    }
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

    args.add(context.image_name(Container::EveBox));
    args.extend(&["evebox", "server"]);

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
    let interfaces = evectl::system::get_interfaces().unwrap();
    let current_if = context.config.suricata.interfaces.first();
    let index = interfaces
        .iter()
        .position(|interface| Some(&interface.name) == current_if)
        .unwrap_or(0);

    let mut selections = evectl::prompt::Selections::with_index();

    for interface in &interfaces {
        let address = interface
            .addr4
            .first()
            .map(|s| format!("-- {}", s.green().italic()))
            .unwrap_or("".to_string());
        selections.push(
            interface.name.to_string(),
            format!("{} {}", &interface.name, address),
        );
    }

    match inquire::Select::new("Select interface", selections.to_vec())
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
    for image in [
        context.image_name(Container::Suricata),
        context.image_name(Container::EveBox),
    ] {
        if let Err(err) = context.manager.pull(&image) {
            error!("Failed to pull {image}: {err}");
            ok = false;
        }
    }
    if let Err(err) = selfupdate::self_update() {
        error!("Failed to update Simple-IDS: {err}");
        ok = false;
    }
    ok
}

fn remove(context: &Context) {
    info!("Stopping Suricata...");
    if let Err(err) = context.manager.stop(SURICATA_CONTAINER_NAME, None) {
        error!("Failed to stop Suricata: {}", err.to_string().trim());
    }
    info!("Stopping EveBox...");
    if let Err(err) = context.manager.stop(EVEBOX_CONTAINER_NAME, None) {
        error!("Failed to stop EveBox: {}", err.to_string().trim());
    }
    info!("Removing Suricata container");
    context.manager.quiet_rm(SURICATA_CONTAINER_NAME);
    info!("Removing EveBox container");
    context.manager.quiet_rm(EVEBOX_CONTAINER_NAME);

    let volumes = [
        "simple-ids-evebox-lib",
        "simple-ids-suricata-lib",
        "simple-ids-suricata-log",
        "simple-ids-suricata-run",
    ];
    for volume in &volumes {
        info!("Removing volume {volume}");
        match context
            .manager
            .command()
            .args(["volume", "rm", volume])
            .status()
        {
            Ok(_status) => {}
            Err(err) => {
                error!("Failed to remove volume {volume}: {err}");
            }
        }
    }

    for image in [
        context.image_name(Container::Suricata),
        context.image_name(Container::EveBox),
    ] {
        info!("Removing image {image}");
        match context
            .manager
            .command()
            .args(["image", "rmi", &image])
            .status()
        {
            Ok(_status) => {}
            Err(err) => {
                error!("Failed to remove image {image}: {err}");
            }
        }
    }

    println!();
    info!("Simple-IDS containers and data have been removed.");
    info!("You may now remove the Simple-IDS program and configuration file (simple-ids.toml).");
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
