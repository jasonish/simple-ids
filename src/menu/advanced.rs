// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{container::Container, context::Context, SelectItem};

pub(crate) fn advanced_menu(context: &mut Context) {
    loop {
        crate::term::title("Simple-IDS: Advanced Configuration");

        let suricata_image_name = context.image_name(Container::Suricata);
        let evebox_image_name = context.image_name(Container::EveBox);

        let selections = vec![
            SelectItem::new(
                "suricata",
                format!("Suricata Container: {}", suricata_image_name),
            ),
            SelectItem::new("evebox", format!("EveBox Container: {}", evebox_image_name)),
            SelectItem::new("return", "Return"),
        ];

        match inquire::Select::new("Select container to configure", selections).prompt() {
            Ok(selection) => match selection.tag.as_ref() {
                "suricata" => {
                    set_suricata_image(context, &suricata_image_name);
                }
                "evebox" => {
                    set_evebox_image(context, &evebox_image_name);
                }
                "return" => return,
                _ => unimplemented!(),
            },
            Err(_) => return,
        }
    }
}

fn set_suricata_image(context: &mut Context, default: &str) {
    match inquire::Text::new("Enter Suricata image name")
        .with_default(default)
        .with_help_message("Enter to keep current, ESC to reset to default")
        .prompt()
    {
        Ok(image) => {
            context.config.suricata.image = Some(image);
        }
        Err(_) => {
            context.config.suricata.image = None;
        }
    }
    context.config.save().unwrap();
}

fn set_evebox_image(context: &mut Context, default: &str) {
    match inquire::Text::new("Enter EveBox image name")
        .with_default(default)
        .with_help_message("Enter to keep current, ESC to reset to default")
        .prompt()
    {
        Ok(image) => {
            context.config.evebox.image = Some(image);
        }
        Err(_) => {
            context.config.evebox.image = None;
        }
    }
    context.config.save().unwrap();
}
