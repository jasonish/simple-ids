// Copyright (c) 2021 Jason Ish
//
// Permission is hereby granted, free of charge, to any person
// obtaining a copy of this software and associated documentation
// files (the "Software"), to deal in the Software without
// restriction, including without limitation the rights to use, copy,
// modify, merge, publish, distribute, sublicense, and/or sell copies
// of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT.  IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
// HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
// WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use tempfile::tempfile;
use tracing::{debug, error, info, warn};

fn file_checksum(file: &mut File) -> Result<String> {
    let mut hash = Sha256::new();
    std::io::copy(file, &mut hash)?;
    let hash = hash.finalize();
    Ok(format!("{:x}", hash))
}

fn current_checksum(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    file_checksum(&mut file)
}

pub fn self_update() -> Result<()> {
    let target = std::env!("TARGET");
    let url = format!("https://evebox.org/files/easy/{}/easy", target);
    let hash_url = format!("{}.sha256", url);
    let current_exe = if let Ok(exe) = std::env::current_exe() {
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
    if let Err(err) = std::fs::remove_file(&current_exe) {
        tracing::warn!(
            "Failed to remove current exe: {}: {}",
            current_exe.display(),
            err
        );
    }
    let mut final_exec = std::fs::File::create(&current_exe)?;
    std::io::copy(&mut download_exe, &mut final_exec)?;
    std::fs::set_permissions(&current_exe, std::fs::Permissions::from_mode(0o0755))?;
    warn!("The Easy program has been updated. Please restart.");
    std::process::exit(0);
}

fn download_release(url: &str) -> Result<File> {
    let mut response = reqwest::blocking::get(url)?;
    let mut dest = tempfile()?;
    std::io::copy(&mut response, &mut dest)?;
    dest.seek(SeekFrom::Start(0))?;
    Ok(dest)
}
