// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::{bail, Result};
use tracing::error;

use crate::container::{CommandExt, SuricataContainer};
use crate::{build_evebox_command, EVEBOX_CONTAINER_NAME};
use crate::{Context, SURICATA_CONTAINER_NAME};

pub(crate) fn force_suricata_logrotate(context: &Context) {
    let _ = context
        .manager
        .command()
        .args([
            "exec",
            SURICATA_CONTAINER_NAME,
            "logrotate",
            "-fv",
            "/etc/logrotate.d/suricata",
        ])
        .status();
}

pub(crate) fn update_rules(context: &Context) -> Result<()> {
    let container = SuricataContainer::new(context.manager);

    let mut volumes = vec![];

    if let Ok(cdir) = std::env::current_dir() {
        for filename in ["enable.conf", "disable.conf"] {
            if cdir.join(filename).exists() {
                volumes.push(format!(
                    "{}/{}:/etc/suricata/{}",
                    cdir.display(),
                    filename,
                    filename,
                ));
            }
        }
    }

    if let Err(err) = container
        .run()
        .rm()
        .it()
        .args(&["suricata-update", "update-sources"])
        .build()
        .status_ok()
    {
        error!("Rule source update did not complete successfully: {err}");
    }
    if let Err(err) = container
        .run()
        .rm()
        .it()
        .volumes(&volumes)
        .args(&["suricata-update"])
        .build()
        .status_ok()
    {
        error!("Rule update did not complete successfully: {err}");
    }
    Ok(())
}

pub(crate) fn start_evebox(context: &Context) -> Result<()> {
    context.manager.quiet_rm(EVEBOX_CONTAINER_NAME);
    let mut command = build_evebox_command(context, true);
    let output = command.output()?;
    if !output.status.success() {
        bail!(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}

pub(crate) fn stop_evebox(context: &Context) -> Result<()> {
    context.manager.stop(EVEBOX_CONTAINER_NAME, Some("SIGINT"))
}
