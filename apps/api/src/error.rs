use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use anneal_core::ApplicationError;

#[derive(Debug)]
pub struct ApiError(pub ApplicationError);

#[derive(Serialize)]
pub struct ErrorResponse {
    pub message: String,
}

impl From<ApplicationError> for ApiError {
    fn from(value: ApplicationError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            ApplicationError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApplicationError::Forbidden => StatusCode::FORBIDDEN,
            ApplicationError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApplicationError::Conflict(_) => StatusCode::CONFLICT,
            ApplicationError::NotFound(_) => StatusCode::NOT_FOUND,
            ApplicationError::Infrastructure(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(ErrorResponse {
                message: self.0.to_string(),
            }),
        )
            .into_response()
    }
}
