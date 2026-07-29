// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use cedar_policy::{EntityId, EntityTypeName, EntityUid};
use std::{str::FromStr, sync::LazyLock};

pub static REGISTER_SERVER: LazyLock<EntityUid> = LazyLock::new(|| {
    EntityUid::from_type_name_and_id(
        EntityTypeName::from_str("Sprue::Action").expect("valid entity type name"),
        EntityId::new("registerServer"),
    )
});
