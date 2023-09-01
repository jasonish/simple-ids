// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub fn getuid() -> u32 {
    return unsafe { libc::getuid() as u32 };
}

// Maybe needed later...
#[allow(dead_code)]
pub fn getgid() -> u32 {
    return unsafe { libc::getgid() as u32 };
}
