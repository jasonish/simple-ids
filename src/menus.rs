// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    actions, add_index, prompt, term, Context, SelectItem, EVEBOX_CONTAINER_NAME,
    SURICATA_CONTAINER_NAME,
};

pub(crate) fn other(context: &Context) {
    loop {
        term::title("Simple-IDS: Other Menu Items");

        let selections = vec![
            SelectItem::new("rotate", "Force Log Rotation"),
            SelectItem::new("suricata-shell", "Suricata Shell"),
            SelectItem::new("evebox-shell", "EveBox Shell"),
            SelectItem::new("return", "Return"),
        ];
        let selections = add_index(&selections);
        match inquire::Select::new("Select menu option", selections).prompt() {
            Err(_) => return,
            Ok(selection) => match selection.tag.as_ref() {
                "return" => return,
                "rotate" => {
                    actions::force_suricata_logrotate(context);
                    prompt::enter();
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
                _ => {}
            },
        }
    }
}
