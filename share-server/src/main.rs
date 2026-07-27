use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

const DEFAULT_BIND: &str = "127.0.0.1:3000";
const BASE_URL: &str = "https://krasava.xyz/api/share";

#[derive(Clone)]
struct AppState {
    shares: Arc<Mutex<HashMap<String, ShareEntry>>>,
}

#[derive(Clone, Serialize)]
struct ShareEntry {
    content: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    max_uses: u32,
    use_count: u32,
}

#[derive(Deserialize)]
struct CreateShare {
    content: String,
    ttl_minutes: u32,
    max_uses: u32,
}

#[derive(Serialize)]
struct ShareResponse {
    token: String,
    url: String,
    expires_at: String,
}

#[derive(Serialize)]
struct ShareInfo {
    token: String,
    url: String,
    expires_at: String,
    use_count: u32,
    max_uses: u32,
    remaining: u32,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = AppState {
        shares: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/api/share", post(create_share))
        .route("/api/share/{token}", get(get_share))
        .route("/api/share/{token}/info", get(share_info))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let bind = std::env::var("SHARE_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    tracing::info!("Listening on {}", bind);
    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_share(
    State(state): State<AppState>,
    Json(body): Json<CreateShare>,
) -> Result<Json<ShareResponse>, (StatusCode, String)> {
    if body.content.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "content is empty".into()));
    }
    let ttl = body.ttl_minutes.max(1).min(60);
    let now = Utc::now();
    let expires_at = now + chrono::Duration::minutes(ttl as i64);

    let token = Uuid::new_v4().to_string();
    let entry = ShareEntry {
        content: body.content,
        created_at: now,
        expires_at,
        max_uses: body.max_uses,
        use_count: 0,
    };

    let mut shares = state.shares.lock().await;
    shares.insert(token.clone(), entry);

    tracing::info!("Created share {} (ttl={}m, max_uses={})", token, ttl, body.max_uses);

    Ok(Json(ShareResponse {
        url: format!("{}/{}", BASE_URL, token),
        token: token.clone(),
        expires_at: expires_at.to_rfc3339(),
    }))
}

async fn get_share(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    let mut shares = state.shares.lock().await;
    let entry = shares.get_mut(&token).ok_or((StatusCode::NOT_FOUND, "Not found"))?;

    if Utc::now() > entry.expires_at {
        shares.remove(&token);
        return Err((StatusCode::GONE, "Link expired"));
    }

    if entry.max_uses > 0 && entry.use_count >= entry.max_uses {
        shares.remove(&token);
        return Err((StatusCode::GONE, "Max uses reached"));
    }

    entry.use_count += 1;
    let content = entry.content.clone();

    if entry.max_uses > 0 && entry.use_count >= entry.max_uses {
        shares.remove(&token);
    }

    Ok(([(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")], content))
}

async fn share_info(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<ShareInfo>, (StatusCode, &'static str)> {
    let shares = state.shares.lock().await;
    let entry = shares.get(&token).ok_or((StatusCode::NOT_FOUND, "Not found"))?;

    let remaining = if entry.max_uses > 0 {
        entry.max_uses.saturating_sub(entry.use_count)
    } else {
        u32::MAX
    };

    Ok(Json(ShareInfo {
        url: format!("{}/{}", BASE_URL, token),
        token: token.clone(),
        expires_at: entry.expires_at.to_rfc3339(),
        use_count: entry.use_count,
        max_uses: entry.max_uses,
        remaining,
    }))
}
