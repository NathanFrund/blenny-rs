mod test_utils;
use futures::StreamExt;
use test_utils::{
    TestUser, create_test_client, get_test_server, get_test_server_with_config, login_and_get_cookie, make_authenticated_request,
};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

#[tokio::test]
async fn login_sets_cookie_and_redirects() {
    let server = get_test_server().await;
    let client = create_test_client();
    let user = TestUser::default();

    let response = client
        .post(&format!("{}/login", server.base_url()))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "username={}&password={}",
            user.username, user.password
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 303); // redirect
    let cookies = response.cookies().collect::<Vec<_>>();
    let token_cookie = cookies.iter().find(|c| c.name() == "blenny_token");
    assert!(token_cookie.is_some());
}

#[tokio::test]
async fn dashboard_accessible_with_cookie() {
    let server = get_test_server().await;
    let client = create_test_client();
    let user = TestUser::default();

    // Login and get auth cookie
    let auth_cookie = login_and_get_cookie(&client, &server.base_url(), &user).await;

    // Request dashboard with cookie
    let dashboard_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/dashboard", server.base_url()),
        &auth_cookie,
    )
    .await;

    assert_eq!(dashboard_response.status().as_u16(), 200);
    let body = dashboard_response.text().await.unwrap();
    assert!(body.contains("Welcome, admin"));
}

#[tokio::test]
async fn dashboard_without_cookie_redirects_to_login() {
    let server = get_test_server().await;
    let client = create_test_client();

    let response = client
        .get(&format!("{}/dashboard", server.base_url()))
        .send()
        .await
        .unwrap();

    // Should redirect (303) to /login
    assert_eq!(response.status().as_u16(), 303);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.contains("/login"));
}

#[tokio::test]
async fn logout_clears_cookie() {
    let server = get_test_server().await;
    let client = create_test_client();
    let user = TestUser::default();

    // Login and get auth cookie
    let auth_cookie = login_and_get_cookie(&client, &server.base_url(), &user).await;

    // Logout
    let logout_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/logout", server.base_url()),
        &auth_cookie,
    )
    .await;

    assert_eq!(logout_response.status().as_u16(), 303);
}

#[tokio::test]
async fn panic_route_returns_json_error() {
    let server = get_test_server().await;
    let client = create_test_client();
    let user = TestUser::default();

    // First authenticate to access protected routes
    let auth_cookie = login_and_get_cookie(&client, &server.base_url(), &user).await;

    let response = client
        .get(&format!("{}/panic", server.base_url()))
        .header("Cookie", &auth_cookie)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 500);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"]["type"], "Internal");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Request handler panicked")
    );
}

#[tokio::test]
async fn protected_routes_require_auth() {
    let server = get_test_server().await;
    let client = create_test_client();

    // Test /direct-to-me requires auth (should redirect)
    let response = client
        .get(&format!("{}/direct-to-me", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 303); // redirect to login

    // Test /panic requires auth (should redirect)
    let response = client
        .get(&format!("{}/panic", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 303); // redirect to login
}

#[tokio::test]
async fn ws_receives_broadcast() {
    let server = get_test_server().await;
    let client = create_test_client();
    let user = TestUser::default();

    // Login and get auth cookie
    let auth_cookie = login_and_get_cookie(&client, &server.base_url(), &user).await;

    // Connect WebSocket with auth
    let url = format!("ws://127.0.0.1:{}/ws", server.port());
    let mut request = url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Cookie", auth_cookie.parse().unwrap());
    let (ws_stream, _) = connect_async(request).await.unwrap();
    let (_write, mut read) = ws_stream.split();

    // Trigger a broadcast (route is protected, but we are authenticated)
    let trigger_url = format!(
        "http://127.0.0.1:{}/trigger-broadcast?category=ui",
        server.port()
    );
    let _resp = client
        .get(&trigger_url)
        .header("Cookie", &auth_cookie)
        .send()
        .await
        .unwrap();

    // Read message from WebSocket
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), read.next())
        .await
        .expect("Timeout waiting for WS message")
        .expect("WS stream ended")
        .expect("WS message error");

    if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
        assert!(text.contains("Broadcasted ui"));
    } else {
        panic!("Expected text message");
    }
}

#[tokio::test]
async fn sse_rejects_unauthenticated_by_default() {
    let server = get_test_server().await;
    let client = create_test_client();
    let resp = client
        .get(&format!("{}/sse", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn ws_unauthenticated_connection_when_auth_not_required() {
    let config = blenny::BlennyConfig {
        websocket: true,
        transport_auth_required: false,
        ..blenny::BlennyConfig::default()
    };
    let server = get_test_server_with_config(config).await;

    // Connect WebSocket without auth
    let url = format!("ws://127.0.0.1:{}/ws", server.port());
    let (ws_stream, _) = connect_async(url).await.unwrap();
    let (_write, mut read) = ws_stream.split();

    // Verify it stays open by waiting for 2 seconds and expecting a timeout (no close or messages)
    let res = tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await;
    assert!(res.is_err(), "Expected timeout because connection should stay open, but got: {:?}", res);
}

#[tokio::test]
async fn ws_survives_lagged_receiver() {
    let config = blenny::BlennyConfig {
        websocket: true,
        transport_auth_required: false,
        ..blenny::BlennyConfig::default()
    };
    let server = get_test_server_with_config(config).await;
    let app_state = server.app_state().await;

    // Connect WebSocket
    let url = format!("ws://127.0.0.1:{}/ws", server.port());
    let (ws_stream, _) = connect_async(url).await.unwrap();
    let (_write, mut read) = ws_stream.split();

    // Trigger more than the buffer size of broadcasts to cause the receiver to lag.
    // The default buffer size is 256. We will broadcast 300 messages.
    for i in 0..300 {
        app_state.hub.broadcast(blenny::transport::ServerMessage {
            category: "test".to_string(),
            html: Some(format!("msg {}", i)),
            signals: None,
        });
    }

    // Now, let's wait a moment for the channel to be filled and cause a lag
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Let's send one more fresh message
    app_state.hub.broadcast(blenny::transport::ServerMessage {
        category: "test".to_string(),
        html: Some("fresh msg".to_string()),
        signals: None,
    });

    // The websocket connection should stay open, and we should be able to read
    // the "fresh msg" (or some subsequent message) eventually because the loop continues
    // after the lag warning is logged.
    let mut got_fresh = false;
    let timeout_dur = std::time::Duration::from_secs(5);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout_dur {
        if let Some(Ok(msg)) = tokio::time::timeout(std::time::Duration::from_millis(500), read.next())
            .await
            .ok()
            .flatten()
        {
            if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                if text.contains("fresh msg") {
                    got_fresh = true;
                    break;
                }
            }
        } else {
            // If the connection was closed, read.next() returns None immediately or error
            break;
        }
    }

    assert!(got_fresh, "Expected to successfully receive fresh message after lag without connection closing");
}

#[tokio::test]
async fn ws_authenticated_connection_via_query_param() {
    let config = blenny::BlennyConfig {
        websocket: true,
        transport_auth_required: true,
        ..blenny::BlennyConfig::default()
    };
    let server = get_test_server_with_config(config).await;

    // Login to get a valid token
    let client = create_test_client();
    let user = TestUser::default();
    let cookie_str = login_and_get_cookie(&client, &server.base_url(), &user).await;
    let token = cookie_str.strip_prefix("blenny_token=").unwrap();

    // Connect WebSocket using the token as a query parameter
    let url = format!("ws://127.0.0.1:{}/ws?token={}", server.port(), token);
    let (ws_stream, _) = connect_async(url).await.unwrap();
    let (_write, mut read) = ws_stream.split();

    // Verify it stays open by waiting for 2 seconds and expecting a timeout
    let res = tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await;
    assert!(res.is_err(), "Expected timeout because connection should stay open, but got: {:?}", res);
}
