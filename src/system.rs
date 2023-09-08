// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use std::{ffi::CStr, process::Command};

pub(crate) fn getuid() -> u32 {
    unsafe { libc::getuid() as u32 }
}

pub(crate) fn _get_username() -> Option<String> {
    unsafe {
        let username = libc::getlogin();
        if username.is_null() {
            None
        } else {
            let username = CStr::from_ptr(username);
            let username = username.to_owned().to_string_lossy().into_owned();
            Some(username.to_string())
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Interface {
    pub name: String,
    pub status: String,
    pub addr4: Vec<String>,
    pub addr6: Vec<String>,
}

/// Get the network interfaces and their addresses.
///
/// We parse the output of the "ip" command as we may need to do this
/// by executing a command in a Docker container.
///
/// Note: Newer versions of "ip" support JSON output.
pub(crate) fn get_interfaces() -> Result<Vec<Interface>> {
    let output = Command::new("ip")
        .args(["--brief", "address", "show"])
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let mut interfaces = vec![];
    for line in stdout.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(' ').filter(|part| !part.is_empty()).collect();
        let name = &parts[0];
        let status = &parts[1];
        let mut interface = Interface {
            name: name.to_string(),
            status: status.to_string(),
            ..Default::default()
        };
        for addr in &parts[2..] {
            let addr = addr.split('/').next().unwrap_or(addr);
            if addr.contains('.') {
                interface.addr4.push(addr.to_string());
            } else {
                interface.addr6.push(addr.to_string());
            }
        }
        interfaces.push(interface);
    }
    Ok(interfaces)
}
