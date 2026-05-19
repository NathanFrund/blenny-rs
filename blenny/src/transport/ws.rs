// blenny/src/transport/ws.rs
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Extension, Query},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::app_state::AppState;

use super::{parse_intents, select_message};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let (do_filter, intents) = parse_intents(&params);

    let mut user = crate::auth::User::from_headers(&headers, &state.jwt_secret);

    if user.is_none() {
        if let Some(token) = params.get("token").or(params.get("blenny_token")) {
            user = crate::auth::User::from_token(token, &state.jwt_secret);
        }
    }

    if state.config.transport_auth_required && user.is_none() {
        return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
    }

    ws.on_failed_upgrade(|error| {
        tracing::error!("WebSocket upgrade failed: {}", error);
    })
    .on_upgrade(move |socket| handle_ws(socket, state, intents, do_filter, user))
}

async fn handle_ws(
    socket: WebSocket,
    state: Arc<AppState>,
    intents: HashSet<String>,
    do_filter: bool,
    user: Option<crate::auth::User>,
) {
    let (mut sender, mut receiver) = socket.split();
    let user_id_for_log = user.as_ref().map(|u| u.id.clone());

    let send_task = async move {
        let mut global_rx = state.hub.subscribe();

        // Register user and hold the guard inside the task
        let (mut personal_rx, _ws_guard) = if let Some(ref user) = user {
            match state.hub.register_user(&user.id) {
                Ok((rx, handle)) => (Some(rx), Some(handle)),
                Err(e) => {
                    tracing::error!("Failed to register user for WS: {}", e);
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

            if do_filter && !intents.contains(msg.category.as_str()) {
                continue;
            }

            let payload = msg.html.or(msg.signals).unwrap_or_default();
            if sender.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    };

    let recv_task = async { while let Some(Ok(_)) = receiver.next().await {} };

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    tracing::debug!("WebSocket disconnected for user: {:?}", user_id_for_log);
}
