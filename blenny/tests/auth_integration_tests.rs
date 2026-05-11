mod test_utils;
use test_utils::{get_test_server, create_test_client, login_and_get_cookie, TestUser, make_authenticated_request};

#[tokio::test]
async fn login_sets_cookie_and_redirects() {
    let server = get_test_server().await;
    let client = create_test_client();
    let user = TestUser::default();

    let response = client
        .post(&format!("{}/login", server.base_url()))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("username={}&password={}", user.username, user.password))
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
    ).await;

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
    ).await;

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
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Request handler panicked"));
}
