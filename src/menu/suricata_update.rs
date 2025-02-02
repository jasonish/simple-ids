// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    container::{CommandExt, Container, RunCommandBuilder},
    context::Context,
    term,
};
use anyhow::Result;
use colored::Colorize;
use std::{io::Write, path::PathBuf};
use tracing::error;

/// Suricata configure menu.
pub(crate) fn menu(context: &mut Context) -> Result<()> {
    loop {
        term::title("Simple-IDS: Configure Suricata-Update");

        let selections = evectl::prompt::Selections::with_index()
            .push("enable-conf", "Edit enable.conf")
            .push("disable-conf", "Edit disable.conf")
            .push("modify-conf", "Edit modify.conf")
            .push("enable-ruleset", "Enable a Ruleset")
            .push("disable-ruleset", "Disable a Ruleset")
            .push("return", "Return")
            .to_vec();

        match inquire::Select::new("Select menu option", selections).prompt() {
            Ok(selection) => match selection.tag {
                "disable-conf" => edit_file(context, "disable.conf"),
                "enable-conf" => edit_file(context, "enable.conf"),
                "modify-conf" => edit_file(context, "modify.conf"),
                "enable-ruleset" => enable_ruleset(context).unwrap(),
                "disable-ruleset" => disable_ruleset(context).unwrap(),
                _ => break,
            },
            Err(_) => break,
        }
    }

    Ok(())
}

fn disable_ruleset(context: &Context) -> Result<()> {
    let enabled = crate::actions::get_enabled_ruleset(context).unwrap();
    if enabled.is_empty() {
        evectl::prompt::enter_with_prefix("No rulesets enabled");
        return Ok(());
    }

    let index = crate::actions::load_rule_index(context).unwrap();
    let mut selections = evectl::prompt::Selections::new();

    for (id, source) in &index.sources {
        if enabled.contains(id) {
            let message = format!("{}: {}", id, source.summary.green().italic());
            selections.push(id, message);
        }
    }

    if let Ok(selection) = inquire::Select::new(
        "Choose a ruleset to DISABLE or ESC to exit",
        selections.to_vec(),
    )
    .with_page_size(16)
    .prompt()
    {
        let _ = crate::actions::disable_ruleset(context, selection.tag);

        if evectl::prompt::confirm(
            "Would you like to update your rules now?",
            Some("A rule update is required to complete disabling this ruleset"),
        ) {
            crate::actions::update_rules(context)?;
        }

        evectl::prompt::enter();
    }

    Ok(())
}

fn enable_ruleset(context: &Context) -> Result<()> {
    let index = crate::actions::load_rule_index(context).unwrap();
    let enabled = crate::actions::get_enabled_ruleset(context).unwrap();
    let mut selections = evectl::prompt::Selections::new();

    for (id, source) in &index.sources {
        if source.obsolete.is_some() {
            continue;
        }
        if source.parameters.is_some() {
            continue;
        }
        if enabled.contains(id) {
            continue;
        }

        let message = format!("{}: {}", id, source.summary.green().italic());

        selections.push(id, message);
    }

    if let Ok(selection) = inquire::Select::new(
        "Choose a ruleset to enable or ESC to exit",
        selections.to_vec(),
    )
    .with_page_size(16)
    .prompt()
    {
        let _ = crate::actions::enable_ruleset(context, selection.tag);

        if evectl::prompt::confirm(
            "Would you like to update your rules now?",
            Some("A rule update is require to make the new ruleset active"),
        ) {
            crate::actions::update_rules(context)?;
        }

        evectl::prompt::enter();
    }

    Ok(())
}

fn copy_suricata_update_template(context: &Context, filename: &str) -> Result<()> {
    let source = format!(
        "/usr/lib/suricata/python/suricata/update/configs/{}",
        filename
    );
    let image = context.image_name(Container::Suricata);
    let output = RunCommandBuilder::new(context.manager, image)
        .rm()
        .args(&["cat", &source])
        .build()
        .status_output()?;
    let mut target = std::fs::File::create(filename)?;
    target.write_all(&output)?;
    Ok(())
}

fn edit_file(context: &Context, filename: &str) {
    let path = PathBuf::from(filename);
    if !path.exists() {
        if let Ok(true) = inquire::Confirm::new(&format!(
            "Would you like to start with a {} template",
            filename
        ))
        .with_default(true)
        .prompt()
        {
            if let Err(err) = copy_suricata_update_template(context, filename) {
                error!(
                    "Sorry, an error occurred copying the template for {}: {}",
                    filename, err
                );
                evectl::prompt::enter();
            }
        }
    }
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".into());
    if let Err(err) = std::process::Command::new(&editor).arg(filename).status() {
        error!("Failed to load {} in editor {}: {}", filename, editor, err);
    }
}
