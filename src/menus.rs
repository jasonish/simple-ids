// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{actions, context::Context, term, EVEBOX_CONTAINER_NAME, SURICATA_CONTAINER_NAME};

pub(crate) fn other(context: &Context) {
    loop {
        term::title("Simple-IDS: Other Menu Items");

        let selections = evectl::prompt::Selections::with_index()
            .push("rotate", "Force Log Rotation")
            .push("suricata-shell", "Suricata Shell")
            .push("evebox-shell", "EveBox Shell")
            .push("remove", "Remove Simple-IDS data")
            .push("return", "Return")
            .to_vec();

        match inquire::Select::new("Select menu option", selections).prompt() {
            Err(_) => return,
            Ok(selection) => match selection.tag {
                "return" => return,
                "rotate" => {
                    actions::force_suricata_logrotate(context);
                    evectl::prompt::enter();
                }
                "suricata-shell" => {
                    let _ = context
                        .manager
                        .command()
                        .args([
                            "exec",
                            "-it",
                            "-e",
                            "PS1=[\\u@suricata \\W]\\$ ",
                            SURICATA_CONTAINER_NAME,
                            "bash",
                        ])
                        .status();
                }
                "evebox-shell" => {
                    let _ = context
                        .manager
                        .command()
                        .args([
                            "exec",
                            "-it",
                            "-e",
                            "PS1=[\\u@evebox \\W]\\$ ",
                            EVEBOX_CONTAINER_NAME,
                            "/bin/sh",
                        ])
                        .status();
                }
                "remove" => {
                    if inquire::Confirm::new("Are you sure you want to remove Simple-IDS data?")
                        .with_default(false)
                        .prompt_skippable()
                        .unwrap()
                        .unwrap_or(false)
                    {
                        crate::remove(context);
                        std::process::exit(0);
                    }
                }
                _ => {}
            },
        }
    }
}
