// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{context::Context, term};

pub(crate) fn menu(context: &mut Context) {
    loop {
        term::title("Simple IDS: Configure Suricata");

        let current_bpf = if let Some(bpf) = &context.config.suricata.bpf {
            format!(" [{}]", bpf)
        } else {
            "".to_string()
        };

        let mut selections = evectl::prompt::Selections::with_index();
        selections.push("bpf-filter", format!("BPF filter{}", current_bpf));
        selections.push("return", "Return");

        match inquire::Select::new("Select an option", selections.to_vec()).prompt() {
            Ok(selection) => match selection.tag {
                "bpf-filter" => set_bpf_filter(context),
                _ => return,
            },
            Err(_) => return,
        }
    }
}

fn set_bpf_filter(context: &mut Context) {
    let default = context
        .config
        .suricata
        .bpf
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or("".to_string());
    if let Ok(filter) = inquire::Text::new("Enter BPF filter")
        .with_default(&default)
        .prompt()
    {
        context.config.suricata.bpf = Some(filter);
        context.config.save().unwrap();
    }
}
