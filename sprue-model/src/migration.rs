// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sqlx::{PgPool, migrate::Migrator};

static MIGRATOR: Migrator = sqlx::migrate!("../sprue-model/migrations");

pub async fn run_migrations(url: &str) -> Result<(), anyhow::Error> {
    let sqlx_pool = PgPool::connect(url).await?;
    run_migrations_on_conn(url, &sqlx_pool).await?;
    Ok(())
}

pub async fn run_migrations_on_conn(url: &str, pool: &PgPool) -> Result<(), anyhow::Error> {
    v_model::migrations::run_migrations(url);
    MIGRATOR.run(pool).await?;
    Ok(())
}
