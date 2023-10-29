// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::{
    env,
    fs::{self, File},
    io::{self, Seek, SeekFrom},
    os::unix::prelude::PermissionsExt,
    path::Path,
    process,
};

use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

// Ok, the return type is a bit odd as this handles a lot of the error
// handling itself. An `Err` is an error that should be logged by the
// caller.  Ok(true) is success, but Ok(false) is an error that was
// logged by this function.
pub(crate) fn self_update() -> Result<()> {
    // If we're running from cargo, don't self update.
    if env::var("CARGO").is_ok() {
        info!("Not self updating as we are running from Cargo");
        return Ok(());
    }

    let target = env!("TARGET");
    let url = format!("https://evebox.org/files/simplensm/{}/simplensm", target);
    let hash_url = format!("{}.sha256", url);
    let current_exe = if let Ok(exe) = env::current_exe() {
        exe
    } else {
        bail!("Failed to determine executable name, cannot self-update");
    };

    info!("Current running executable: {}", current_exe.display());

    info!("Calculating checksum of current executable");
    let current_hash = match current_checksum(&current_exe) {
        Err(err) => {
            tracing::warn!("Failed to calculate checksum of current exec: {}", err);
            None
        }
        Ok(checksum) => Some(checksum),
    };

    info!("Downloading {}", &hash_url);
    let response = reqwest::blocking::get(&hash_url)?;
    if response.status().as_u16() != 200 {
        error!(
            "Failed to fetch remote checksum: HTTP status code={}",
            response.status(),
        );
        return Ok(());
    }
    let remote_hash = response.text()?.trim().to_lowercase();
    debug!("Remote SHA256 checksum: {}", &remote_hash);

    match current_hash {
        None => {
            info!("Failed to determine checksum of current exe, updating");
        }
        Some(checksum) => {
            if checksum != remote_hash {
                info!("Remote checksum different than current exe, will update");
            } else {
                info!("No update available");
                return Ok(());
            }
        }
    }

    info!("Downloading {}", &url);
    let mut download_exe = download_release(&url)?;

    // Verify the checksum.
    let hash = file_checksum(&mut download_exe)?;
    debug!(
        "Locally calculated SHA256 checksum for downloaded file: {}",
        &hash
    );
    if hash != remote_hash {
        tracing::error!("Downloaded file has invalid checksum, not updating");
        tracing::error!("- Expected {}", remote_hash);
        return Ok(());
    }

    info!("Replacing current executable");
    download_exe.seek(SeekFrom::Start(0))?;
    if let Err(err) = fs::remove_file(&current_exe) {
        tracing::warn!(
            "Failed to remove current exe: {}: {}",
            current_exe.display(),
            err
        );
    }
    let mut final_exec = fs::File::create(&current_exe)?;
    io::copy(&mut download_exe, &mut final_exec)?;
    fs::set_permissions(&current_exe, fs::Permissions::from_mode(0o0755))?;
    warn!("The SimleNSM program has been updated. Please restart.");
    process::exit(0);
}

fn download_release(url: &str) -> Result<File> {
    let mut response = reqwest::blocking::get(url)?;
    let mut dest = tempfile::tempfile()?;
    io::copy(&mut response, &mut dest)?;
    dest.seek(SeekFrom::Start(0))?;
    Ok(dest)
}

fn file_checksum(file: &mut File) -> Result<String> {
    let mut hash = Sha256::new();
    io::copy(file, &mut hash)?;
    let hash = hash.finalize();
    Ok(format!("{:x}", hash))
}

fn current_checksum(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    file_checksum(&mut file)
}
