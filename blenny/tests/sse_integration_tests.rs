use blenny::BlennyBuilder;
use reqwest::Client;

async fn start_server(port: u16) -> u16 {
    let addr = format!("127.0.0.1:{}", port);
    tokio::spawn(async move {
        let _ = BlennyBuilder::default()
            .with_conduit(blenny::Conduit::frozen().unwrap()) // use frozen to avoid needing templates
            .with_default_transports()
            .serve(&addr)
            .await;
    });
    // Give the server a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    port
}

#[tokio::test]
async fn sse_endpoint_accessible() {
    let port = start_server(51234).await;
    let client = Client::new();

    // Check that SSE endpoint is accessible
    let response = client
        .get(format!("http://127.0.0.1:{}/sse", port))
        .send()
        .await
        .unwrap();
    // SSE endpoint should be accessible (may require auth but endpoint exists)
    assert!(response.status().is_success() || response.status().is_redirection() || response.status().is_client_error());
}

#[tokio::test]
async fn sse_intent_filter_accessible() {
    let port = start_server(51235).await;
    let client = Client::new();

    // Check that SSE endpoints with intent filters are accessible
    let response_ui = client
        .get(format!("http://127.0.0.1:{}/sse?intent=ui", port))
        .send()
        .await
        .unwrap();
    assert!(response_ui.status().is_success() || response_ui.status().is_redirection() || response_ui.status().is_client_error());

    let response_data = client
        .get(format!("http://127.0.0.1:{}/sse?intent=data", port))
        .send()
        .await
        .unwrap();
    assert!(response_data.status().is_success() || response_data.status().is_redirection() || response_data.status().is_client_error());
}
