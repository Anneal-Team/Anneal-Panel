use axum::http::HeaderMap;

use anneal_auth::AccessClaims;
use anneal_core::{Actor, ApplicationError, ApplicationResult};

use crate::app_state::AppState;

pub fn authenticated_actor(headers: &HeaderMap, state: &AppState) -> ApplicationResult<Actor> {
    let claims = bearer_claims(headers, state)?;
    if claims.kind != "access" {
        return Err(ApplicationError::Unauthorized);
    }
    Ok(Actor {
        user_id: claims.sub,
        tenant_id: claims.tenant_id,
        role: claims.role,
    })
}

pub fn pre_auth_claims(headers: &HeaderMap, state: &AppState) -> ApplicationResult<AccessClaims> {
    let claims = bearer_claims(headers, state)?;
    if claims.kind != "pre_auth" && claims.kind != "access" {
        return Err(ApplicationError::Unauthorized);
    }
    Ok(claims)
}

fn bearer_claims(headers: &HeaderMap, state: &AppState) -> ApplicationResult<AccessClaims> {
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .ok_or(ApplicationError::Unauthorized)?;
    let token = authorization
        .strip_prefix("Bearer ")
        .ok_or(ApplicationError::Unauthorized)?;
    state.auth_service().decode_claims(token)
}
