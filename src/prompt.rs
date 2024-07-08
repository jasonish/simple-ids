// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub(crate) fn enter() {
    let _ = inquire::Text::new("Press ENTER to continue:").prompt();
}

pub(crate) fn enter_with_prefix(prefix: &str) {
    let _ = inquire::Text::new(&format!("{}. Press ENTER to continue:", prefix)).prompt();
}

pub(crate) fn confirm(prompt: &str, help: Option<&str>) -> bool {
    let prompt = inquire::Confirm::new(prompt);
    let prompt = if let Some(help) = help {
        prompt.with_help_message(help)
    } else {
        prompt
    };
    matches!(prompt.prompt(), Ok(true))
}
