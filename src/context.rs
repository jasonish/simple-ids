// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    config::Config,
    container::{Container, ContainerManager, DEFAULT_EVEBOX_IMAGE, DEFAULT_SURICATA_IMAGE},
};

#[derive(Clone)]
pub(crate) struct Context {
    pub config: Config,
    pub manager: ContainerManager,
}

impl Context {
    /// Given a container type, return the image name.
    ///
    /// Normally this will be the hardcoded default, but we do allow
    /// it to be overridden in the configuration.
    pub(crate) fn image_name(&self, container: Container) -> String {
        match container {
            Container::Suricata => self
                .config
                .suricata
                .image
                .as_deref()
                .unwrap_or(DEFAULT_SURICATA_IMAGE)
                .to_string(),
            Container::EveBox => self
                .config
                .evebox
                .image
                .as_deref()
                .unwrap_or(DEFAULT_EVEBOX_IMAGE)
                .to_string(),
        }
    }
}
