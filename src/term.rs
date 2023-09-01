// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::io::{stdin, stdout, Write};
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};

// Override the default print! macro to flush the output.
macro_rules! print {
    ($fmt:expr) => {
        #[allow(clippy::explicit_write)]
        write!(std::io::stdout(), $fmt).unwrap();
        std::io::stdout().flush().unwrap()
    };
    ($fmt:expr, $( $arg:expr ),*) => {
        #[allow(clippy::explicit_write)]
        write!(std::io::stdout(), $fmt, $($arg),*).unwrap();
        std::io::stdout().flush().unwrap()
    };
}

pub fn read_line() -> String {
    let mut s = String::new();
    stdin().read_line(&mut s).unwrap();
    s.trim().to_string()
}

pub fn prompt_for_enter() {
    print!("Press ENTER to continue: ");
    let _ = stdout().flush();
    let _ = read_line();
}

pub fn print_err(msg: &str) {
    let mut stdout = StandardStream::stdout(termcolor::ColorChoice::Always);
    stdout
        .set_color(ColorSpec::new().set_fg(Some(termcolor::Color::Red)))
        .unwrap();
    println!("{}", msg);
    stdout.set_color(ColorSpec::new().set_fg(None)).unwrap();
}

pub fn print_status(label: &str, value: &str) {
    let value_color = Color::Rgb(0x00, 0xbf, 0xff);
    let mut stdout = StandardStream::stdout(termcolor::ColorChoice::Always);
    print!("{}: ", label);
    stdout
        .set_color(ColorSpec::new().set_fg(Some(value_color)))
        .unwrap();
    println!("{}", value);
    stdout.set_color(ColorSpec::new().set_fg(None)).unwrap();
}

pub fn dummy_prompt(prompt: &str) {
    print!("{}", prompt);
    let _ = read_line();
}
