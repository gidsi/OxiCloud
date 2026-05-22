use std::{future::Future, pin::Pin};

use crate::domain::entities::user::User;

pub trait UserRepository {
    fn find_by_session_token<'a>(
        &'a self,
        token: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<User>, sqlx::Error>> + Send + 'a>>;
}
