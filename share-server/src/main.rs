use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
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

/// Максимум ссылок с одного IP в минуту
const RATE_LIMIT_PER_MIN: u32 = 5;
/// Максимум всего активных ссылок
const MAX_TOTAL_SHARES: usize = 500;

fn client_ip(headers: &HeaderMap) -> String {
    if let Some(val) = headers.get("x-forwarded-for") {
        if let Ok(s) = val.to_str() {
            if let Some(ip) = s.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }
    "unknown".to_string()
}

fn is_rate_limited(history: &mut HashMap<String, Vec<DateTime<Utc>>>, ip: &str) -> bool {
    let now = Utc::now();
    let window = now - chrono::Duration::minutes(1);
    let times = history.entry(ip.to_string()).or_default();
    times.retain(|t| *t > window);
    if times.len() >= RATE_LIMIT_PER_MIN as usize {
        return true;
    }
    times.push(now);
    false
}

#[derive(Clone)]
struct AppState {
    shares: Arc<Mutex<HashMap<String, ShareEntry>>>,
    rate_history: Arc<Mutex<HashMap<String, Vec<DateTime<Utc>>>>>,
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
        rate_history: Arc::new(Mutex::new(HashMap::new())),
    };

    // Фоновая чистка просроченных ссылок каждые 5 минут
    let shares_cleanup = state.shares.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            let mut shares = shares_cleanup.lock().await;
            let before = shares.len();
            shares.retain(|_, e| Utc::now() <= e.expires_at && (e.max_uses == 0 || e.use_count < e.max_uses));
            let removed = before - shares.len();
            if removed > 0 {
                tracing::info!("Cleaned up {} expired shares ({} active)", removed, shares.len());
            }
        }
    });

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
    headers: HeaderMap,
    Json(body): Json<CreateShare>,
) -> Result<Json<ShareResponse>, (StatusCode, String)> {
    if body.content.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "content is empty".into()));
    }

    // Rate limit по IP
    let ip = client_ip(&headers);
    {
        let mut rate = state.rate_history.lock().await;
        if is_rate_limited(&mut rate, &ip) {
            tracing::warn!("Rate limit hit for {}", ip);
            return Err((StatusCode::TOO_MANY_REQUESTS, "Слишком много запросов. Попробуйте через минуту.".into()));
        }
    }

    // Глобальный лимит активных ссылок
    {
        let shares = state.shares.lock().await;
        if shares.len() >= MAX_TOTAL_SHARES {
            return Err((StatusCode::SERVICE_UNAVAILABLE, "Сервер переполнен, попробуйте позже.".into()));
        }
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

    tracing::info!("Created share {} (ttl={}m, max_uses={}) from {}", token, ttl, body.max_uses, ip);

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
