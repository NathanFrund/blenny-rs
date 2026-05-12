// blenny/tests/surrealdb_integration.rs
mod test_utils;

#[cfg(feature = "surreal")]
mod surrealdb_tests {
    use crate::test_utils::get_test_server_with_config;
    use blenny::BlennyConfig;
    use std::time::Duration;
    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::Ws; // <-- Required for Surreal::new::<Ws>
    use surrealdb::opt::auth::Root;
    use tokio::time::timeout;

    fn clean_url(raw: Option<String>) -> Option<String> {
        raw.map(|url| {
            if let Some(stripped) = url.strip_prefix("ws://") {
                stripped.to_string()
            } else if let Some(stripped) = url.strip_prefix("wss://") {
                stripped.to_string()
            } else {
                url
            }
        })
    }

    fn db_url() -> Option<String> {
        clean_url(std::env::var("SURREALDB_URL").ok())
    }

    async fn setup_surrealdb() -> Result<(String, String), String> {
        let url = db_url().ok_or("SURREALDB_URL not set")?;

        eprintln!("Connecting to SurrealDB at {url}...");
        let db = timeout(Duration::from_secs(5), Surreal::new::<Ws>(url.clone()))
            .await
            .map_err(|_| format!("Timed out connecting to {url}"))?
            .map_err(|e| format!("Connection error: {e}"))?;

        eprintln!("Signing in as root...");
        timeout(
            Duration::from_secs(5),
            db.signin(Root {
                username: "root".to_string(),
                password: "root".to_string(),
            }),
        )
        .await
        .map_err(|_| "Timed out during sign in".to_string())?
        .map_err(|e| format!("Sign in error: {e}"))?;

        let ns = format!("test_ns_{}", rand::random::<u32>());
        let db_name = format!("test_db_{}", rand::random::<u32>());

        eprintln!("Creating namespace {ns}...");
        db.query(format!("DEFINE NAMESPACE IF NOT EXISTS {ns}"))
            .await
            .map_err(|e| format!("Error creating namespace: {e}"))?;

        eprintln!("Creating database {db_name}...");
        db.query(format!("DEFINE DATABASE IF NOT EXISTS {db_name}"))
            .await
            .map_err(|e| format!("Error creating database: {e}"))?;

        eprintln!("Using ns/db...");
        db.use_ns(&ns)
            .use_db(&db_name)
            .await
            .map_err(|e| format!("Error switching to ns/db: {e}"))?;

        eprintln!("Setup complete.");
        Ok((ns, db_name))
    }

    async fn teardown_surrealdb(ns: &str, db_name: &str) {
        let url = db_url().unwrap();
        let db = match timeout(Duration::from_secs(5), Surreal::new::<Ws>(url)).await {
            Ok(Ok(db)) => db,
            _ => return,
        };
        let _ = timeout(
            Duration::from_secs(5),
            db.signin(Root {
                username: "root".to_string(),
                password: "root".to_string(),
            }),
        )
        .await;
        let _ = db.query(format!("REMOVE DATABASE {db_name}")).await;
        let _ = db.query(format!("REMOVE NAMESPACE {ns}")).await;
    }

    #[tokio::test]
    async fn create_schema_insert_and_query() {
        if db_url().is_none() {
            eprintln!("Skipping test: SURREALDB_URL not set");
            return;
        }
        let (ns, db_name) = setup_surrealdb().await.expect("SurrealDB setup failed");

        let config = BlennyConfig {
            database_url: std::env::var("SURREALDB_URL").ok(),
            ..BlennyConfig::default()
        };
        let server = get_test_server_with_config(config).await;
        let app_state = server.app_state().await;
        let db = app_state
            .surrealdb
            .as_ref()
            .expect("SurrealDB client not found");
        db.use_ns(&ns).use_db(&db_name).await.unwrap();

        db.query("CREATE person SET name = 'Nathan', age = 30")
            .await
            .unwrap();
        let mut result = db.query("SELECT * FROM person").await.unwrap();
        let people: Vec<serde_json::Value> = result.take(0).unwrap();
        assert_eq!(people.len(), 1);

        let person = &people[0];
        assert_eq!(person["name"].as_str(), Some("Nathan"));
        assert_eq!(person["age"].as_i64(), Some(30));

        teardown_surrealdb(&ns, &db_name).await;
    }

    #[tokio::test]
    async fn query_through_builder_injected_client() {
        if db_url().is_none() {
            eprintln!("Skipping test: SURREALDB_URL not set");
            return;
        }
        let (ns, db_name) = setup_surrealdb().await.expect("SurrealDB setup failed");

        let config = BlennyConfig {
            database_url: std::env::var("SURREALDB_URL").ok(),
            ..BlennyConfig::default()
        };
        let server = get_test_server_with_config(config).await;
        let app_state = server.app_state().await;
        let db = app_state.surrealdb.as_ref().unwrap();
        db.use_ns(&ns).use_db(&db_name).await.unwrap();

        db.query("CREATE person SET name = 'Tobie'").await.unwrap();
        db.query("CREATE person SET name = 'Jaime'").await.unwrap();

        let people: Vec<serde_json::Value> = db.select("person").await.unwrap();
        assert_eq!(people.len(), 2);

        teardown_surrealdb(&ns, &db_name).await;
    }
}
