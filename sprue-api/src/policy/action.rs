use std::{str::FromStr, sync::LazyLock};

use cedar_policy::{EntityId, EntityTypeName, EntityUid};

pub static REGISTER_SERVER: LazyLock<EntityUid> = LazyLock::new(|| {
    EntityUid::from_type_name_and_id(
        EntityTypeName::from_str("Sprue::Action").expect("valid entity type name"),
        EntityId::new("registerServer"),
    )
});
