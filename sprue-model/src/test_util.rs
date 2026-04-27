// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::migration::run_migrations;
use nanoid::nanoid;
use sqlx::{PgPool, query};

static ALPHABET: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn leakable_dbs() -> Vec<String> {
    let leaks = std::env::var("LEAK_TEST_DB").unwrap_or_else(|_| String::new());
    leaks.split(',').map(|s| s.to_string()).collect()
}

// A fresh test database that will be created and migrated for use in a test. At the end of the
// the test (or when the database is dropped) the database will be deleted
#[derive(Debug)]
pub struct TestDb {
    pub db_base: String,
    pub db_name: String,
    pub should_drop: bool,
}

impl TestDb {
    pub async fn new(test_name: &str) -> Self {
        let db_base = std::env::var("TEST_DATABASE").expect(
            "TEST_DATABASE environment variable must be specified to run integration tests",
        );
        let db_name = format!("sprue_t_{}_{}", test_name, nanoid!(8, &ALPHABET));

        let should_drop = !leakable_dbs().iter().any(|s| s == test_name);

        let db = Self {
            db_base,
            db_name,
            should_drop,
        };

        let raw_query = format!("CREATE DATABASE {}", db.db_name);
        tracing::debug!(db.db_name, "Creating test database");

        let q = query(&raw_query);
        q.execute(&db.base_conn()).await.unwrap();
        tracing::debug!(db.db_name, "Created test database");

        run_migrations(&db.url()).await.unwrap();
        tracing::debug!(db.db_name, "Populated test database");

        db
    }

    pub fn url(&self) -> String {
        format!("{}/{}", self.db_base, self.db_name)
    }

    fn base_conn(&self) -> PgPool {
        PgPool::connect_lazy(&self.db_base).unwrap()
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        if self.should_drop {
            tracing::info!("Dropping test database {}", self.db_name);

            let base_conn = self.base_conn();
            let drop_query = format!("DROP DATABASE {} WITH (FORCE)", self.db_name);
            let result = std::thread::spawn(|| {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(async move {
                        query(&drop_query).execute(&base_conn).await.unwrap();
                    });
            })
            .join();
            println!("{:#?}", result);
        }
    }
}
