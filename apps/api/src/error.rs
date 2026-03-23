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
        let (status, message) = match self.0 {
            ApplicationError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, String::from("unauthorized"))
            }
            ApplicationError::Forbidden => (StatusCode::FORBIDDEN, String::from("forbidden")),
            ApplicationError::Validation(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
            ApplicationError::Conflict(message) => (StatusCode::CONFLICT, message),
            ApplicationError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            ApplicationError::Infrastructure(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("internal server error"),
            ),
        };
        (status, Json(ErrorResponse { message })).into_response()
    }
}
