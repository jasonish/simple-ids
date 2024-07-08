// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;

use crate::{add_index, context::Context, term, SelectItem};

/// Main configure menu.
pub(crate) fn main(context: &mut Context) -> Result<()> {
    loop {
        term::title("Simple-IDS: Configure");

        let selections = vec![
            SelectItem::new("suricata", "Suricata Configuration"),
            SelectItem::new("suricata-update", "Suricata-Update Configuration"),
            SelectItem::new("evebox", "EveBox Configuration"),
            SelectItem::new("advanced", "Advanced"),
            SelectItem::new("return", "Return"),
        ];
        let selections = add_index(&selections);

        match inquire::Select::new("Select menu option", selections).prompt() {
            Ok(selection) => match selection.tag.as_ref() {
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
