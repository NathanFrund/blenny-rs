#[cfg(feature = "surreal")]
mod surrealdb_tests {
    use blenny::BlennyConfig;

    async fn get_server_with_db() {
        let config = BlennyConfig {
            database_url: Some("ws://localhost:8000".into()),
            ..BlennyConfig::default()
        };
        crate::test_utils::get_test_server_with_config(config).await
    }

    #[tokio::test]
    #[ignore] // Requires running SurrealDB instance
    async fn test_surrealdb_connection() {
        let _server = get_server_with_db().await;
        // Make a request to a handler that queries the DB and verify response
    }
}