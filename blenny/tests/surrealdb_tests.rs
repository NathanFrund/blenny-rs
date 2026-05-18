mod test_utils;

#[cfg(feature = "surreal")]
mod surrealdb_tests {
    use crate::test_utils;
    use blenny::BlennyConfig;

    async fn get_server_with_db() -> test_utils::TestServer {
        let config = BlennyConfig {
            database_url: Some("ws://localhost:8000".into()),
            ..BlennyConfig::default()
        };
        test_utils::get_test_server_with_config(config).await
    }

    #[tokio::test]
    #[ignore] // Requires running SurrealDB instance
    async fn test_surrealdb_connection() {
        let _server = get_server_with_db().await;
        // Make a request to a handler that queries the DB and verify response
    }
}
