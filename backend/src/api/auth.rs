use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::models::ApiResponse;
use crate::services::auth_service::AuthUser;
use crate::state::AppState;

pub const SESSION_COOKIE: &str = "wsm_session";
pub const CSRF_COOKIE: &str = "wsm_csrf";
pub const CSRF_HEADER: &str = "x-csrf-token";
pub const PERM_VIEW_DASHBOARD: i64 = 1 << 0;
pub const PERM_MANAGE_SERVER: i64 = 1 << 1;
pub const PERM_MANAGE_CONFIG: i64 = 1 << 2;
pub const PERM_MANAGE_BACKUPS: i64 = 1 << 3;
pub const PERM_MANAGE_INSTALL: i64 = 1 << 4;
pub const PERM_MANAGE_UPDATES: i64 = 1 << 5;
pub const PERM_MANAGE_SCHEDULE: i64 = 1 << 6;
pub const PERM_MANAGE_USERS: i64 = 1 << 7;

#[derive(Debug, Serialize)]
pub struct AuthStatus {
    pub has_users: bool,
    pub needs_bootstrap: bool,
}

#[derive(Debug, Deserialize)]
pub struct BootstrapRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub username: String,
    pub is_admin: bool,
    pub permission_flags: i64,
}

#[derive(Debug, Serialize)]
pub struct AuthUserSummary {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub permission_flags: i64,
    pub disabled: bool,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct InviteSummary {
    pub id: i64,
    pub permission_flags: i64,
    pub max_uses: i64,
    pub uses: i64,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub created_by_user: Option<i64>,
    pub exhausted: bool,
    pub expired: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub permission_flags: i64,
    pub max_uses: Option<i64>,
    pub expires_in_hours: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CreatedInvite {
    pub code: String,
    pub permission_flags: i64,
    pub max_uses: i64,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterWithInviteRequest {
    pub invite_code: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateResetCodeRequest {
    pub username: String,
    pub expires_in_minutes: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub is_admin: Option<bool>,
    pub permission_flags: Option<i64>,
    pub disabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CreatedResetCode {
    pub code: String,
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub reset_code: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct AuditEventSummary {
    pub id: i64,
    pub created_at: i64,
    pub actor_user_id: Option<i64>,
    pub actor_username: Option<String>,
    pub action: String,
    pub details: Option<String>,
    pub success: bool,
}

pub async fn status(State(app): State<AppState>) -> (StatusCode, Json<ApiResponse<AuthStatus>>) {
    match app.auth.has_users() {
        Ok(has_users) => {
            let status = AuthStatus {
                has_users,
                needs_bootstrap: !has_users,
            };
            (StatusCode::OK, Json(ApiResponse::ok(status)))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(e))),
    }
}

pub async fn bootstrap(
    State(app): State<AppState>,
    Json(req): Json<BootstrapRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match app.auth.bootstrap_admin(&req.username, &req.password) {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(()))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ApiResponse::err(e))),
    }
}

pub async fn login(
    State(app): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Response {
    match app.auth.login(&req.username, &req.password) {
        Ok((token, user)) => {
            let csrf_token = csrf_token_for_session(&token);
            let mut res = Json(ApiResponse::ok(SessionInfo {
                username: user.username,
                is_admin: user.is_admin,
                permission_flags: user.permission_flags,
            }))
            .into_response();
            res.headers_mut().append(
                header::SET_COOKIE,
                session_cookie(
                    &token,
                    app.config.tls_enabled,
                    app.config.auth_session_ttl_secs,
                ),
            );
            res.headers_mut().append(
                header::SET_COOKIE,
                csrf_cookie(
                    &csrf_token,
                    app.config.tls_enabled,
                    app.config.auth_session_ttl_secs,
                ),
            );
            res
        }
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<SessionInfo>::err(e)),
        )
            .into_response(),
    }
}

pub async fn logout(State(app): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = extract_cookie(&headers, SESSION_COOKIE) {
        let _ = app.auth.logout(&token);
    }

    let mut res = Json(ApiResponse::ok(())).into_response();
    res.headers_mut()
        .append(header::SET_COOKIE, clear_session_cookie(app.config.tls_enabled));
    res.headers_mut()
        .append(header::SET_COOKIE, clear_csrf_cookie(app.config.tls_enabled));
    res
}

pub async fn me(State(app): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = extract_cookie(&headers, SESSION_COOKIE) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<SessionInfo>::err("Not authenticated")),
        )
            .into_response();
    };

    match app.auth.validate_session(&token) {
        Ok(Some(user)) => Json(ApiResponse::ok(SessionInfo {
            username: user.username,
            is_admin: user.is_admin,
            permission_flags: user.permission_flags,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<SessionInfo>::err("Session expired or invalid")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<SessionInfo>::err(e)),
        )
            .into_response(),
    }
}

pub async fn list_users(State(app): State<AppState>) -> Response {
    match app.auth.list_users() {
        Ok(users) => Json(ApiResponse::ok(
            users
                .into_iter()
                .map(|u| AuthUserSummary {
                    id: u.id,
                    username: u.username,
                    is_admin: u.is_admin,
                    permission_flags: u.permission_flags,
                    disabled: u.disabled,
                    created_at: u.created_at,
                })
                .collect::<Vec<_>>(),
        ))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<AuthUserSummary>>::err(e)),
        )
            .into_response(),
    }
}

pub async fn update_user(
    State(app): State<AppState>,
    Path(user_id): Path<i64>,
    Extension(request_user): Extension<RequestUser>,
    Json(req): Json<UpdateUserRequest>,
) -> Response {
    match app.auth.update_user(
        request_user.id,
        user_id,
        req.is_admin,
        req.permission_flags,
        req.disabled,
    ) {
        Ok(user) => Json(ApiResponse::ok(AuthUserSummary {
            id: user.id,
            username: user.username,
            is_admin: user.is_admin,
            permission_flags: user.permission_flags,
            disabled: user.disabled,
            created_at: user.created_at,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<AuthUserSummary>::err(e)),
        )
            .into_response(),
    }
}

pub async fn create_invite(
    State(app): State<AppState>,
    Extension(request_user): Extension<RequestUser>,
    Json(req): Json<CreateInviteRequest>,
) -> Response {
    let max_uses = req.max_uses.unwrap_or(1).clamp(1, 1000);
    let expires_at = req.expires_in_hours.and_then(|h| {
        if h <= 0 {
            None
        } else {
            Some(chrono::Utc::now().timestamp() + (h * 60 * 60))
        }
    });

    match app.auth.create_invite(
        Some(request_user.id),
        req.permission_flags,
        max_uses,
        expires_at,
    ) {
        Ok(code) => Json(ApiResponse::ok(CreatedInvite {
            code,
            permission_flags: req.permission_flags,
            max_uses,
            expires_at,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CreatedInvite>::err(e)),
        )
            .into_response(),
    }
}

pub async fn list_invites(State(app): State<AppState>) -> Response {
    let now = chrono::Utc::now().timestamp();
    match app.auth.list_invites() {
        Ok(invites) => Json(ApiResponse::ok(
            invites
                .into_iter()
                .map(|i| InviteSummary {
                    id: i.id,
                    permission_flags: i.permission_flags,
                    max_uses: i.max_uses,
                    uses: i.uses,
                    created_at: i.created_at,
                    expires_at: i.expires_at,
                    created_by_user: i.created_by_user,
                    exhausted: i.uses >= i.max_uses,
                    expired: i.expires_at.map(|ts| ts <= now).unwrap_or(false),
                })
                .collect::<Vec<_>>(),
        ))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<InviteSummary>>::err(e)),
        )
            .into_response(),
    }
}

pub async fn register_with_invite(
    State(app): State<AppState>,
    Json(req): Json<RegisterWithInviteRequest>,
) -> Response {
    match app
        .auth
        .register_with_invite(&req.invite_code, &req.username, &req.password)
    {
        Ok(()) => Json(ApiResponse::ok(())).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::err(e)),
        )
            .into_response(),
    }
}

pub async fn create_reset_code(
    State(app): State<AppState>,
    Extension(request_user): Extension<RequestUser>,
    Json(req): Json<CreateResetCodeRequest>,
) -> Response {
    let ttl_minutes = req.expires_in_minutes.unwrap_or(30).clamp(5, 24 * 60);
    let expires_at = chrono::Utc::now().timestamp() + (ttl_minutes * 60);

    match app
        .auth
        .create_reset_code(Some(request_user.id), &req.username, expires_at)
    {
        Ok(code) => Json(ApiResponse::ok(CreatedResetCode { code, expires_at })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CreatedResetCode>::err(e)),
        )
            .into_response(),
    }
}

pub async fn reset_password(
    State(app): State<AppState>,
    Json(req): Json<ResetPasswordRequest>,
) -> Response {
    match app.auth.reset_password(&req.reset_code, &req.new_password) {
        Ok(()) => Json(ApiResponse::ok(())).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::err(e)),
        )
            .into_response(),
    }
}

pub async fn list_audit(
    State(app): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> Response {
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    match app.auth.list_audit_events(limit) {
        Ok(events) => Json(ApiResponse::ok(
            events
                .into_iter()
                .map(|e| AuditEventSummary {
                    id: e.id,
                    created_at: e.created_at,
                    actor_user_id: e.actor_user_id,
                    actor_username: e.actor_username,
                    action: e.action,
                    details: e.details,
                    success: e.success,
                })
                .collect::<Vec<_>>(),
        ))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<AuditEventSummary>>::err(e)),
        )
            .into_response(),
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct RequestUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub permission_flags: i64,
}

pub async fn require_auth(
    State(app): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    // Public routes during bootstrap/login.
    if is_public_route(&path) {
        return next.run(req).await;
    }

    // Until first account is created, allow legacy behavior and setup access.
    match app.auth.has_users() {
        Ok(false) => return next.run(req).await,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::err(format!("Auth status error: {e}"))),
            )
                .into_response();
        }
        Ok(true) => {}
    }

    let token = match extract_cookie(req.headers(), SESSION_COOKIE) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<()>::err("Authentication required")),
            )
                .into_response();
        }
    };

    match app.auth.validate_session(&token) {
        Ok(Some(user)) => {
            if is_csrf_protected_method(&method)
                && !is_csrf_exempt_route(&path)
                && !validate_csrf(req.headers(), &token)
            {
                return (
                    StatusCode::FORBIDDEN,
                    Json(ApiResponse::<()>::err("CSRF token validation failed")),
                )
                    .into_response();
            }

            if let Err(msg) = authorize_request(&user, &method, &path) {
                return (
                    StatusCode::FORBIDDEN,
                    Json(ApiResponse::<()>::err(msg)),
                )
                    .into_response();
            }

            req.extensions_mut().insert(RequestUser {
                id: user.id,
                username: user.username,
                is_admin: user.is_admin,
                permission_flags: user.permission_flags,
            });
            next.run(req).await
        }
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err("Session expired or invalid")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::err(format!("Auth validation error: {e}"))),
        )
            .into_response(),
    }
}

pub fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let mut kv = part.trim().splitn(2, '=');
        let key = kv.next()?.trim();
        let value = kv.next()?.trim();
        if key == name {
            return Some(value.to_string());
        }
    }
    None
}

pub fn session_cookie(token: &str, secure: bool, max_age_secs: i64) -> HeaderValue {
    let max_age_secs = max_age_secs.clamp(60, 60 * 60 * 24 * 30);
    let secure_attr = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={token}; HttpOnly; Path=/; SameSite=Strict; Max-Age={max_age_secs}{secure_attr}"
    ))
    .expect("valid Set-Cookie header")
}

pub fn csrf_token_for_session(token: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn csrf_cookie(token: &str, secure: bool, max_age_secs: i64) -> HeaderValue {
    let max_age_secs = max_age_secs.clamp(60, 60 * 60 * 24 * 30);
    let secure_attr = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{CSRF_COOKIE}={token}; Path=/; SameSite=Strict; Max-Age={max_age_secs}{secure_attr}"
    ))
    .expect("valid CSRF Set-Cookie header")
}

pub fn clear_session_cookie(secure: bool) -> HeaderValue {
    if secure {
        HeaderValue::from_static(
            "wsm_session=; HttpOnly; Path=/; SameSite=Strict; Max-Age=0; Secure",
        )
    } else {
        HeaderValue::from_static(
            "wsm_session=; HttpOnly; Path=/; SameSite=Strict; Max-Age=0",
        )
    }
}

pub fn clear_csrf_cookie(secure: bool) -> HeaderValue {
    if secure {
        HeaderValue::from_static("wsm_csrf=; Path=/; SameSite=Strict; Max-Age=0; Secure")
    } else {
        HeaderValue::from_static("wsm_csrf=; Path=/; SameSite=Strict; Max-Age=0")
    }
}

pub fn is_public_route(path: &str) -> bool {
    matches!(
        path,
        "/api/health"
            | "/api/setup/status"
            | "/api/auth/status"
            | "/api/auth/bootstrap"
            | "/api/auth/login"
            | "/api/auth/logout"
            | "/api/auth/register"
            | "/api/auth/reset-password"
            | "/"
    )
}

pub fn has_permission(user: &AuthUser, required_permission: i64) -> bool {
    user.is_admin || (user.permission_flags & required_permission) == required_permission
}

pub fn is_csrf_protected_method(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

pub fn is_csrf_exempt_route(path: &str) -> bool {
    matches!(
        path,
        "/api/auth/login"
            | "/api/auth/bootstrap"
            | "/api/auth/register"
            | "/api/auth/reset-password"
            | "/api/auth/logout"
    )
}

pub fn validate_csrf(headers: &HeaderMap, session_token: &str) -> bool {
    let expected = csrf_token_for_session(session_token);
    let cookie_token = extract_cookie(headers, CSRF_COOKIE);
    let header_token = headers
        .get(CSRF_HEADER)
        .or_else(|| headers.get("x-xsrf-token"))
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim().to_string());

    match (cookie_token, header_token) {
        (Some(cookie), Some(header)) => {
            !cookie.is_empty() && cookie == header && header == expected
        }
        _ => false,
    }
}

pub fn required_permission_for(method: &Method, path: &str) -> Option<i64> {
    if path == "/ws" {
        return Some(PERM_VIEW_DASHBOARD);
    }

    if path == "/api/state"
        || path == "/api/logs"
        || path == "/api/players"
        || path == "/api/history/players"
        || path == "/api/server/stats"
    {
        return Some(PERM_VIEW_DASHBOARD);
    }

    if path.starts_with("/api/server/") {
        return Some(PERM_MANAGE_SERVER);
    }

    if path.starts_with("/api/config/") {
        if method == Method::GET {
            return Some(PERM_VIEW_DASHBOARD);
        }
        return Some(PERM_MANAGE_CONFIG);
    }

    if path.starts_with("/api/setup/") {
        return Some(PERM_MANAGE_CONFIG);
    }

    if path == "/api/backup" && method == Method::GET {
        return Some(PERM_VIEW_DASHBOARD);
    }
    if path.starts_with("/api/backup/") {
        return Some(PERM_MANAGE_BACKUPS);
    }

    if path == "/api/schedule" && method == Method::GET {
        return Some(PERM_VIEW_DASHBOARD);
    }
    if path.starts_with("/api/schedule") {
        return Some(PERM_MANAGE_SCHEDULE);
    }

    if path == "/api/install" && method == Method::GET {
        return Some(PERM_VIEW_DASHBOARD);
    }
    if path.starts_with("/api/install/") {
        return Some(PERM_MANAGE_INSTALL);
    }

    if path == "/api/update" && method == Method::GET {
        return Some(PERM_VIEW_DASHBOARD);
    }
    if path.starts_with("/api/update/") {
        return Some(PERM_MANAGE_UPDATES);
    }

    if path == "/api/auth/me" {
        return Some(PERM_VIEW_DASHBOARD);
    }
    if path == "/api/auth/users"
        || path.starts_with("/api/auth/users/")
        || path == "/api/auth/invites"
        || path == "/api/auth/reset-code"
        || path == "/api/auth/audit"
    {
        return Some(PERM_MANAGE_USERS);
    }

    // Default for authenticated routes that are not yet explicitly mapped.
    Some(PERM_VIEW_DASHBOARD)
}

pub fn authorize_request(user: &AuthUser, method: &Method, path: &str) -> Result<(), &'static str> {
    match required_permission_for(method, path) {
        Some(required_permission) if !has_permission(user, required_permission) => {
            Err("Insufficient permissions")
        }
        _ => Ok(()),
    }
}

pub fn validate_ws_auth(app: &AppState, headers: &HeaderMap) -> Result<Option<AuthUser>, String> {
    let has_users = app.auth.has_users()?;
    if !has_users {
        return Ok(Some(AuthUser {
            id: 0,
            username: "bootstrap".to_string(),
            is_admin: true,
            permission_flags: i64::MAX,
        }));
    }
    let Some(token) = extract_cookie(headers, SESSION_COOKIE) else {
        return Ok(None);
    };
    let user = app.auth.validate_session(&token)?;
    Ok(user.filter(|u| has_permission(u, PERM_VIEW_DASHBOARD)))
}
