// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{add_index, term, Context, SelectItem};

/// Main configure menu.
pub(crate) fn main(context: &mut Context) {
    loop {
        term::clear_title("SimpleNSM: Configure");

        let selections = vec![
            SelectItem::new("suricata-update", "Suricata-Update Configuration"),
            SelectItem::new("evebox", "EveBox Configuration"),
            SelectItem::new("return", "Return"),
        ];
        let selections = add_index(&selections);

        match inquire::Select::new("Select menu option", selections).prompt() {
            Ok(selection) => match selection.tag.as_ref() {
                "suricata-update" => crate::menu::suricata_update::menu(context),
                "evebox" => crate::menu::evebox::configure(context),
                "return" => return,
                _ => unimplemented!(),
            },
            Err(_) => return,
        }
    }
}
