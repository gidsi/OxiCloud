use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct HealthCheckResponse {
    pub status: &'static str,
    pub database: &'static str,
}

impl HealthCheckResponse {
    pub const fn pass() -> Self {
        Self {
            status: "pass",
            database: "connected",
        }
    }

    pub const fn fail() -> Self {
        Self {
            status: "fail",
            database: "disconnected",
        }
    }
}
