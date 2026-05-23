use axum::{
    extract::State,
    http::{header, HeaderName, HeaderValue, StatusCode},
    Json,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::{Duration, Instant},
};

use crate::{
    application::dtos::health::HealthCheckResponse,
    infrastructure::state::AppState,
};

const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);
const HEALTH_PASS_CACHE_TTL: Duration = Duration::from_secs(1);

struct CachedHealthPass {
    state: Weak<AppState>,
    expires_at: Instant,
}

static HEALTH_PASS_CACHE: OnceLock<Mutex<HashMap<usize, CachedHealthPass>>> = OnceLock::new();

pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> (
    StatusCode,
    [(HeaderName, HeaderValue); 1],
    Json<HealthCheckResponse>,
) {
    let cache_key = Arc::as_ptr(&state) as usize;

    let database_connected = if state.db_pool.is_closed() {
        false
    } else if is_cached_pass(&state, cache_key) {
        true
    } else {
        let pool = state.db_pool.clone();

        let check_result = tokio::time::timeout(HEALTH_CHECK_TIMEOUT, async move {
            sqlx::query("SELECT 1").execute(&pool).await
        })
        .await;

        let connected = matches!(check_result, Ok(Ok(_)));

        if connected {
            cache_pass(&state, cache_key);
        }

        connected
    };

    let (status_code, response_body) = if database_connected {
        (StatusCode::OK, HealthCheckResponse::pass())
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            HealthCheckResponse::fail(),
        )
    };

    (
        status_code,
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(response_body),
    )
}

fn is_cached_pass(state: &Arc<AppState>, cache_key: usize) -> bool {
    let Some(cache) = HEALTH_PASS_CACHE.get() else {
        return false;
    };

    let now = Instant::now();

    let Ok(mut cache) = cache.lock() else {
        return false;
    };

    let Some(entry) = cache.get(&cache_key) else {
        return false;
    };

    let valid_for_same_state = entry.expires_at > now
        && entry
            .state
            .upgrade()
            .is_some_and(|cached_state| Arc::ptr_eq(&cached_state, state));

    if valid_for_same_state {
        true
    } else {
        cache.remove(&cache_key);
        false
    }
}

fn cache_pass(state: &Arc<AppState>, cache_key: usize) {
    let cache = HEALTH_PASS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    let Ok(mut cache) = cache.lock() else {
        return;
    };

    let now = Instant::now();

    cache.retain(|_, entry| {
        entry.expires_at > now && entry.state.strong_count() > 0
    });

    cache.insert(
        cache_key,
        CachedHealthPass {
            state: Arc::downgrade(state),
            expires_at: now + HEALTH_PASS_CACHE_TTL,
        },
    );
}
