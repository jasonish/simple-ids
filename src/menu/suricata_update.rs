// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    add_index,
    container::{CommandExt, RunCommandBuilder},
    prompt, term, Context, SelectItem, SURICATA_IMAGE,
};
use anyhow::Result;
use std::{io::Write, path::PathBuf};
use tracing::error;

/// Suricata configure menu.
pub(crate) fn menu(context: &mut Context) {
    loop {
        term::clear_title("SimpleNSM: Configure Suricata");

        let selections = vec![
            SelectItem::new("enable-conf", "Edit enable.conf"),
            SelectItem::new("disable-conf", "Edit disable.conf"),
            SelectItem::new("return", "Return"),
        ];
        let selections = add_index(&selections);

        match inquire::Select::new("Select menu option", selections).prompt() {
            Ok(selection) => match selection.tag.as_ref() {
                "disable-conf" => edit_file(context, "disable.conf"),
                "enable-conf" => edit_file(context, "enable.conf"),
                _ => return,
            },
            Err(_) => return,
        }
    }
}

fn copy_suricata_update_template(context: &Context, filename: &str) -> Result<()> {
    let source = format!(
        "/usr/lib/suricata/python/suricata/update/configs/{}",
        filename
    );
    let output = RunCommandBuilder::new(context.manager, SURICATA_IMAGE)
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
