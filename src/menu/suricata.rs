// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{add_index, context::Context, term, SelectItem};

pub(crate) fn menu(context: &mut Context) {
    loop {
        term::title("Simple IDS: Configure Suricata");

        let current_bpf = if let Some(bpf) = &context.config.suricata.bpf {
            format!(" [{}]", bpf)
        } else {
            "".to_string()
        };

        let selections = vec![
            SelectItem::new("bpf-filter", format!("BPF filter{}", current_bpf)),
            SelectItem::new("return", "Return"),
        ];

        let selections = add_index(&selections);

        match inquire::Select::new("Select an option", selections).prompt() {
            Ok(selection) => match selection.tag.as_ref() {
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
