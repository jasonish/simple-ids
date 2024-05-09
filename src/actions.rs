// SPDX-FileCopyrightText: (C) 2021 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use anyhow::{bail, Result};
use tracing::error;

use crate::container::{CommandExt, SuricataContainer};
use crate::context::Context;
use crate::ruleindex::RuleIndex;
use crate::SURICATA_CONTAINER_NAME;
use crate::{build_evebox_command, EVEBOX_CONTAINER_NAME};

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

pub(crate) fn load_rule_index(context: &Context) -> Result<RuleIndex> {
    let container = SuricataContainer::new(context.clone());
    let output = container
        .run()
        .rm()
        .args(&["cat", "/var/lib/suricata/update/cache/index.yaml"])
        .build()
        .status_output()?;
    let index: RuleIndex = serde_yaml::from_slice(&output)?;
    Ok(index)
}

pub(crate) fn get_enabled_ruleset(context: &Context) -> Result<HashSet<String>> {
    let mut enabled: HashSet<String> = HashSet::new();
    let container = SuricataContainer::new(context.clone());
    let output = container
        .run()
        .args(&["suricata-update", "list-sources", "--enabled"])
        .build()
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let re = regex::Regex::new(r"^[\s]*\-\s*(.*)").unwrap();
    for line in stdout.lines() {
        if let Some(caps) = re.captures(line) {
            enabled.insert(String::from(&caps[1]));
        }
    }
    Ok(enabled)
}

pub(crate) fn enable_ruleset(context: &Context, ruleset: &str) -> Result<()> {
    let container = SuricataContainer::new(context.clone());
    container
        .run()
        .args(&["suricata-update", "enable-source", ruleset])
        .build()
        .status_ok()?;
    Ok(())
}

pub(crate) fn disable_ruleset(context: &Context, ruleset: &str) -> Result<()> {
    let container = SuricataContainer::new(context.clone());
    container
        .run()
        .args(&["suricata-update", "disable-source", ruleset])
        .build()
        .status_ok()?;
    Ok(())
}

pub(crate) fn update_rules(context: &Context) -> Result<()> {
    let container = SuricataContainer::new(context.clone());

    let mut volumes = vec![];

    if let Ok(cdir) = std::env::current_dir() {
        for filename in ["enable.conf", "disable.conf", "modify.conf"] {
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
