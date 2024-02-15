// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{config::Config, container::ContainerManager};

#[derive(Clone)]
pub(crate) struct Context {
    pub config: Config,
    pub manager: ContainerManager,
}
