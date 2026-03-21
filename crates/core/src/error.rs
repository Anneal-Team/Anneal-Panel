use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Infrastructure(String),
}

pub type ApplicationResult<T> = Result<T, ApplicationError>;
