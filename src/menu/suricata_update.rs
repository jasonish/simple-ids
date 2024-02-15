// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    add_index,
    container::{self, CommandExt, Container, RunCommandBuilder},
    context::Context,
    prompt, term, SelectItem,
};
use anyhow::Result;
use colored::Colorize;
use std::{io::Write, path::PathBuf};
use tracing::error;

/// Suricata configure menu.
pub(crate) fn menu(context: &mut Context) {
    loop {
        term::title("Simple-IDS: Configure Suricata-Update");

        let selections = vec![
            SelectItem::new("enable-conf", "Edit enable.conf"),
            SelectItem::new("disable-conf", "Edit disable.conf"),
            SelectItem::new("enable-ruleset", "Enable a Ruleset"),
            SelectItem::new("disable-ruleset", "Disable a Ruleset"),
            SelectItem::new("return", "Return"),
        ];
        let selections = add_index(&selections);

        match inquire::Select::new("Select menu option", selections).prompt() {
            Ok(selection) => match selection.tag.as_ref() {
                "disable-conf" => edit_file(context, "disable.conf"),
                "enable-conf" => edit_file(context, "enable.conf"),
                "enable-ruleset" => enable_ruleset(context),
                "disable-ruleset" => disable_ruleset(context),
                _ => return,
            },
            Err(_) => return,
        }
    }
}

fn disable_ruleset(context: &Context) {
    let enabled = crate::actions::get_enabled_ruleset(context).unwrap();
    if enabled.is_empty() {
        prompt::enter_with_prefix("No rulesets enabled");
        return;
    }

    let index = crate::actions::load_rule_index(context).unwrap();
    let mut selections = vec![];

    for (id, source) in &index.sources {
        if enabled.contains(id) {
            let message = format!("{}: {}", id, source.summary.green().italic());
            selections.push(SelectItem::new(id, message));
        }
    }

    if let Ok(selection) =
        inquire::Select::new("Choose a ruleset to DISABLE or ESC to exit", selections)
            .with_page_size(16)
            .prompt()
    {
        let _ = crate::actions::disable_ruleset(context, &selection.tag);
        prompt::enter();
    }
}

fn enable_ruleset(context: &Context) {
    let index = crate::actions::load_rule_index(context).unwrap();
    let enabled = crate::actions::get_enabled_ruleset(context).unwrap();
    let mut selections = vec![];

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

        selections.push(SelectItem::new(id, message));
    }

    if let Ok(selection) =
        inquire::Select::new("Choose a ruleset to enable or ESC to exit", selections)
            .with_page_size(16)
            .prompt()
    {
        let _ = crate::actions::enable_ruleset(context, &selection.tag);
        prompt::enter();
    }
}

fn copy_suricata_update_template(context: &Context, filename: &str) -> Result<()> {
    let source = format!(
        "/usr/lib/suricata/python/suricata/update/configs/{}",
        filename
    );
    let image = container::image_name(context, Container::Suricata);
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
        if let Ok(true) =
            inquire::Confirm::new(&format!("Would you to start with a {} template", filename))
                .with_default(true)
                .prompt()
        {
            if let Err(err) = copy_suricata_update_template(context, filename) {
                error!(
                    "Sorry, an error occurred copying the template for {}: {}",
                    filename, err
                );
                prompt::enter();
            }
        }
    }
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".into());
    if let Err(err) = std::process::Command::new(&editor).arg(filename).status() {
        error!("Failed to load {} in editor {}: {}", filename, editor, err);
    }
}
