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

    // Stash some image names for easy access.
    pub suricata_image: String,
    pub evebox_image: String,
}

impl Context {
    pub(crate) fn new(config: Config, manager: ContainerManager) -> Self {
        let suricata_image = image_name(&config, Container::Suricata);
        let evebox_image = image_name(&config, Container::EveBox);
        Self {
            config,
            manager,
            suricata_image,
            evebox_image,
        }
    }

    /// Given a container type, return the image name.
    ///
    /// Normally this will be the hardcoded default, but we do allow
    /// it to be overridden in the configuration.
    pub(crate) fn image_name(&self, container: Container) -> String {
        image_name(&self.config, container)
    }
}

/// Given a container type, return the image name.
///
/// Normally this will be the hardcoded default, but we do allow
/// it to be overridden in the configuration.
pub(crate) fn image_name(config: &Config, container: Container) -> String {
    match container {
        Container::Suricata => config
            .suricata
            .image
            .as_deref()
            .unwrap_or(DEFAULT_SURICATA_IMAGE)
            .to_string(),
        Container::EveBox => config
            .evebox
            .image
            .as_deref()
            .unwrap_or(DEFAULT_EVEBOX_IMAGE)
            .to_string(),
    }
}
