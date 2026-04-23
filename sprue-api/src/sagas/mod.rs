// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

pub mod actions;
pub mod background;

use crate::context::ApiContext;

/// This implements steno's SagaType trait to define the context type
/// used by all saga actions.
#[derive(Debug, Clone)]
pub struct SprueSaga;

impl steno::SagaType for SprueSaga {
    type ExecContextType = ApiContext;
}
