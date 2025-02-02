// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;

use crate::{context::Context, term};

/// Main configure menu.
pub(crate) fn main(context: &mut Context) -> Result<()> {
    loop {
        term::title("Simple-IDS: Configure");

        let mut selections = evectl::prompt::Selections::with_index();
        selections.push("suricata", "Suricata Configuration");
        selections.push("suricata-update", "Suricata-Update Configuration");
        selections.push("evebox", "EveBox Configuration");
        selections.push("advanced", "Advanced");
        selections.push("return", "Return");

        match inquire::Select::new("Select menu option", selections.to_vec()).prompt() {
            Ok(selection) => match selection.tag {
                "suricata" => crate::menu::suricata::menu(context),
                "suricata-update" => crate::menu::suricata_update::menu(context)?,
                "evebox" => crate::menu::evebox::configure(context),
                "advanced" => crate::menu::advanced::advanced_menu(context),
                "return" => return Ok(()),
                _ => unimplemented!(),
            },
            Err(_) => break,
        }
    }

    Ok(())
}
