// blenny/src/transport/sse.rs
use axum::{
    extract::{Extension, Query},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    response::{
        Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use crate::app_state::AppState;

use super::select_message;

pub async fn sse_handler(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let mut user = crate::auth::User::from_headers(&headers, &state.jwt_secret);

    if user.is_none() {
        if let Some(token) = params.get("token").or(params.get("blenny_token")) {
            user = crate::auth::User::from_token(token, &state.jwt_secret);
        }
    }

    if state.config.transport_auth_required && user.is_none() {
        return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
    }

    let do_server_filter = !state.encoder.filters_client_side();
    let intent_param = params.get("intent").cloned();
    let do_filter = intent_param.is_some();
    let intents: HashSet<String> = intent_param
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    Sse::new(sse_stream(
        state,
        user,
        do_server_filter,
        do_filter,
        intents,
    ))
    .keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
    .into_response()
}

fn sse_stream(
    state: Arc<AppState>,
    user: Option<crate::auth::User>,
    do_server_filter: bool,
    do_filter: bool,
    intents: HashSet<String>,
) -> impl futures::Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        let mut global_rx = state.hub.subscribe();

        let (mut personal_rx, _connection_guard) = if let Some(ref user) = user {
            match state.hub.register_user(&user.id) {
                Ok((rx, handle)) => (Some(rx), Some(handle)),
                Err(e) => {
                    tracing::error!("Failed to register user for SSE: {}", e);
                    yield Ok(Event::default().comment("failed to register for personal messages"));
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        loop {
            let msg = match select_message(&mut global_rx, &mut personal_rx).await {
                Some(msg) => msg,
                None => break,
            };

            if do_server_filter && do_filter && !intents.contains(msg.category.as_str()) {
                continue;
            }

            let event = state.encoder.to_event(&msg);
            yield Ok(event);
        }
    }
}
