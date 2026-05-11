//! Shared test utilities for Blenny integration tests
//!
//! This module provides common fixtures, helpers, and utilities to make
//! testing easier and more consistent across the codebase.

use blenny::{BlennyBuilder, Conduit};
use reqwest::{Client, redirect};
use std::net::TcpListener;

/// Test server fixture that manages a single server instance
/// for the duration of all integration tests
pub struct TestServer {
    port: u16,
    _handle: tokio::task::JoinHandle<()>,
}

impl TestServer {
    /// Get the port the server is running on
    #[allow(unused)] // May be used in future tests
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the base URL for the server
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Abort the server task when TestServer is dropped
        self._handle.abort();
    }
}

/// Create and start a fresh test server for each test
pub async fn get_test_server() -> TestServer {
    let port = get_random_port();
    let addr = format!("127.0.0.1:{}", port);

    let handle = tokio::spawn(async move {
        // Use frozen Conduit for tests (no hot-reload needed)
        let conduit = Conduit::frozen().unwrap();
        let builder = BlennyBuilder::default()
            .with_conduit(conduit)
            .with_default_transports();

        if let Err(e) = builder.serve(&addr).await {
            eprintln!("Test server error: {}", e);
        }
    });

    // Wait for server to be ready - increased timeout for reliability
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    TestServer {
        port,
        _handle: handle,
    }
}

/// Get a random available port for testing
fn get_random_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Create a test HTTP client with common configuration
pub fn create_test_client() -> Client {
    Client::builder()
        .redirect(redirect::Policy::none())
        .cookie_store(true) // Enable cookie jar
        .build()
        .unwrap()
}

/// Test user credentials for authentication tests
pub struct TestUser {
    pub username: String,
    pub password: String,
}

impl Default for TestUser {
    fn default() -> Self {
        Self {
            username: "admin".to_string(),
            password: "password".to_string(),
        }
    }
}

impl TestUser {
    #[allow(unused)] // May be used for testing different user credentials
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

/// Helper to perform login and return the auth cookie
pub async fn login_and_get_cookie(client: &Client, base_url: &str, user: &TestUser) -> String {
    let response = client
        .post(&format!("{}/login", base_url))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("username={}&password={}", user.username, user.password))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 303); // Should redirect after login

    let cookies = response.cookies().collect::<Vec<_>>();
    let token_cookie = cookies.iter()
        .find(|c| c.name() == "blenny_token")
        .expect("Login should set blenny_token cookie");

    format!("{}={}", token_cookie.name(), token_cookie.value())
}

/// Helper to make authenticated requests
pub async fn make_authenticated_request(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    cookie: &str,
) -> reqwest::Response {
    client
        .request(method, url)
        .header("Cookie", cookie)
        .send()
        .await
        .unwrap()
}

/// Test fixture for SSE connections
#[allow(unused)] // Future SSE integration tests will use this
pub struct SseTestFixture {
    pub client: Client,
    pub base_url: String,
}

#[allow(unused)] // Future SSE integration tests will use this
impl SseTestFixture {
    pub async fn new() -> Self {
        let server = get_test_server().await;
        let client = create_test_client();

        Self {
            client,
            base_url: server.base_url(),
        }
    }

    pub async fn connect_sse(&self, intent: Option<&str>) -> reqwest::Response {
        let url = match intent {
            Some(intent) => format!("{}/sse?intent={}", self.base_url, intent),
            None => format!("{}/sse", self.base_url),
        };

        self.client
            .get(&url)
            .send()
            .await
            .unwrap()
    }
}