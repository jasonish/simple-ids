// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use tracing::{error, info, warn};

use crate::{
    actions, config::EveBoxConfig, container::Container, context::Context, term, ArgBuilder,
    EVEBOX_CONTAINER_NAME,
};

pub(crate) fn configure(context: &mut Context) {
    let original_config = context.config.clone();
    let mut restart_required;
    loop {
        term::title("Simple-IDS: Configure EveBox");

        let is_running = context.manager.is_running(EVEBOX_CONTAINER_NAME);
        restart_required = is_running && original_config != context.config;

        let mut selections = evectl::prompt::Selections::with_index();
        if context.config.evebox.allow_remote {
            selections.push("disable-remote", "Disable Remote Access");
        } else {
            selections.push("enable-remote", "Enable Remote Access");
        }
        selections.push(
            "toggle-tls",
            format!(
                "Toggle TLS (Currently {})",
                if context.config.evebox.no_tls {
                    "disabled"
                } else {
                    "enabled"
                }
            ),
        );
        selections.push(
            "toggle-auth",
            format!(
                "Toggle authentication (Currently {})",
                if context.config.evebox.no_auth {
                    "disabled"
                } else {
                    "enabled"
                }
            ),
        );
        selections.push("reset-password", "Reset Admin Password");
        selections.push(
            "return",
            if restart_required {
                "Restart and Return"
            } else {
                "Return"
            },
        );

        if let Ok(selection) =
            inquire::Select::new("Select menu option", selections.to_vec()).prompt()
        {
            match selection.tag {
                "toggle-tls" => toggle_tls(&mut context.config.evebox),
                "toggle-auth" => toggle_auth(&mut context.config.evebox),
                "reset-password" => reset_password(context),
                "enable-remote" => enable_remote_access(context),
                "disable-remote" => disable_remote_access(context),
                "return" => break,
                _ => {}
            }
        } else {
            break;
        }
    }

    if original_config != context.config {
        info!("Saving configuration changes");
        if let Err(err) = context.config.save() {
            error!("Failed to save configuration changes: {err}");
            evectl::prompt::enter();
        }
    }
    if restart_required {
        info!("Restarting Evebox");
        let _ = actions::stop_evebox(context);
        let _ = actions::start_evebox(context);
    }
}

fn toggle_tls(config: &mut EveBoxConfig) {
    if config.no_tls {
        config.no_tls = false;
    } else {
        if config.allow_remote {
            match inquire::Confirm::new(
                "Remote access is enabled, are you sure you want to disable TLS",
            )
            .with_default(false)
            .prompt()
            {
                Ok(true) => {}
                Ok(false) | Err(_) => return,
            }
        }
        config.no_tls = true;
    }
}

fn toggle_auth(config: &mut EveBoxConfig) {
    if config.no_auth {
        config.no_tls = false;
    } else {
        if config.allow_remote {
            match inquire::Confirm::new(
                "Remote access is enabled, are you sure you want to disable authentication",
            )
            .with_default(false)
            .prompt()
            {
                Ok(true) => {}
                Ok(false) | Err(_) => return,
            }
        }
        config.no_auth = true;
    }
}

fn enable_remote_access(context: &mut Context) {
    if context.config.evebox.no_tls {
        warn!("Enabling TLS");
        context.config.evebox.no_tls = false;
    }
    if context.config.evebox.no_auth {
        warn!("Enabling authentication");
        context.config.evebox.no_auth = false;
    }
    context.config.evebox.allow_remote = true;

    if let Ok(true) = inquire::Confirm::new("Do you wish to reset the admin password")
        .with_default(true)
        .prompt()
    {
        reset_password(context);
    }

    if context.manager.is_running(EVEBOX_CONTAINER_NAME) {
        info!("Restarting EveBox");
        let _ = actions::stop_evebox(context);
        let _ = actions::start_evebox(context);
    }
    evectl::prompt::enter_with_prefix("EveBox remote access has been enabled");
}

fn disable_remote_access(context: &mut Context) {
    context.config.evebox.allow_remote = false;
}

fn reset_password(context: &mut Context) {
    let image = context.image_name(Container::EveBox);
    let mut args = ArgBuilder::new();
    args.add("run");
    for volume in Container::EveBox.volumes() {
        args.add("-v");
        args.add(volume);
    }
    args.extend(&[
        "--rm", "-it", &image, "evebox", "config", "users", "rm", "admin",
    ]);
    let _ = context.manager.command().args(&args.args).status();

    let mut args = ArgBuilder::new();
    args.add("run");
    for volume in Container::EveBox.volumes() {
        args.add("-v");
        args.add(volume);
    }
    args.extend(&[
        "--rm",
        "-it",
        &image,
        "evebox",
        "config",
        "users",
        "add",
        "--username",
        "admin",
    ]);
    let _ = context.manager.command().args(&args.args).status();
}
