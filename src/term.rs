// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crossterm::{
    cursor, execute, style,
    terminal::{Clear, ClearType},
};
use std::io::Write;

pub(crate) fn title(title: &str) {
    let no_clear = std::env::var("NO_CLEAR").map(|_| true).unwrap_or(false);
    if no_clear {
        println!("{}\n", title);
    } else {
        let mut stdout = std::io::stdout().lock();
        let _ = execute!(
            stdout,
            Clear(ClearType::All),
            cursor::MoveTo(0, 0),
            style::Print(title),
            cursor::MoveToNextLine(2)
        );
        let _ = stdout.flush();
    }
}
