use blenny::{BlennyBuilder, Conduit};
use reqwest::{Client, redirect};

async fn start_server(port: u16) -> u16 {
    let addr = format!("127.0.0.1:{}", port);
    tokio::spawn(async move {
        // Use real Conduit for the login form template
        let conduit = Conduit::hot_reload("templates/").unwrap();
        BlennyBuilder::default()
            .with_conduit(conduit)
            .with_default_transports()
            .serve(&addr)
            .await
            .unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    port
}

fn create_client() -> Client {
    Client::builder()
        .redirect(redirect::Policy::none())
        .build()
        .unwrap()
}

#[tokio::test]
async fn login_sets_cookie_and_redirects() {
    let port = start_server(51236).await;
    let client = create_client();

    let response = client
        .post(format!("http://127.0.0.1:{}/login", port))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("username=admin&password=password")
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
    let port = start_server(51237).await;
    let client = create_client();

    // Login first
    let response = client
        .post(format!("http://127.0.0.1:{}/login", port))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("username=admin&password=password")
        .send()
        .await
        .unwrap();

    // Extract cookie
    let cookie = response.cookies().next().unwrap();

    // Request dashboard with cookie
    let dashboard_response = client
        .get(format!("http://127.0.0.1:{}/dashboard", port))
        .header("Cookie", format!("{}={}", cookie.name(), cookie.value()))
        .send()
        .await
        .unwrap();

    assert_eq!(dashboard_response.status().as_u16(), 200);
    let body = dashboard_response.text().await.unwrap();
    assert!(body.contains("Welcome, admin"));
}

#[tokio::test]
async fn dashboard_without_cookie_redirects_to_login() {
    let port = start_server(51238).await;
    let client = create_client();

    let response = client
        .get(format!("http://127.0.0.1:{}/dashboard", port))
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
    let port = start_server(51239).await;
    let client = create_client();

    // Login
    let response = client
        .post(format!("http://127.0.0.1:{}/login", port))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("username=admin&password=password")
        .send()
        .await
        .unwrap();
    let cookie = response.cookies().next().unwrap();

    // Logout
    let logout_response = client
        .get(format!("http://127.0.0.1:{}/logout", port))
        .header("Cookie", format!("{}={}", cookie.name(), cookie.value()))
        .send()
        .await
        .unwrap();

    assert_eq!(logout_response.status().as_u16(), 303);
}
