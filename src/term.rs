// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crossterm::{
    cursor, execute, style,
    terminal::{Clear, ClearType},
};
use is_terminal::IsTerminal;
use std::io::Write;

pub(crate) fn _is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

pub(crate) fn clear_title(title: &str) {
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

/// An alternative to println!() as mixing standard print with fancy
/// terminal actions like prompts doesn't always end up with nice
/// output.
pub(crate) fn _println<S: AsRef<str>>(s: S) {
    let s = s.as_ref();
    let mut stdout = std::io::stdout().lock();
    let _ = execute!(stdout, style::Print(s), cursor::MoveToNextLine(1));
    let _ = stdout.flush();
}
