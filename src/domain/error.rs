use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Unauthorized access")]
    Unauthorized,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Internal server error")]
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let message = self.to_string();

        match self {
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                [(
                    header::WWW_AUTHENTICATE,
                    HeaderValue::from_static(r#"Basic realm="OxiCloud", charset="UTF-8""#),
                )],
                message,
            )
                .into_response(),
            AppError::Database(_) | AppError::Internal => {
                (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
            }
        }
    }
}
