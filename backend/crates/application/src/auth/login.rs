use domain::user::UserRepository;
use std::sync::Arc;
use thiserror::Error;

struct LoginEmailRequestDto {
    pub email: String,
}

struct LoginEmailResponseDto {
    pub jwt: String,
}

fn normalize_email(email: &str) -> Result<String, LoginError> {
    if !email.contains('@') {
        return Err(LoginError::InvalidEmail);
    }
    let (local, domain) = email.split_once('@').unwrap();
    let local = local.split('+').next().unwrap_or(local);
    Ok(format!("{local}@{domain}"))
}

#[derive(Debug, Clone)]
pub struct LoginResult {
    pub id: String,
    pub email: String,
    pub username: String,
}

#[derive(Error, Debug)]
pub enum LoginError {
    #[error("Invalid email")]
    InvalidEmail,
    #[error("User not found")]
    UserNotFound,
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Command to initiate login by email. Transport (oneshot) is handled by the mediator.
#[derive(Debug)]
pub struct LoginEmailCommand {
    pub email: String,
}

pub struct LoginEmailHandler {
    user_repository: Arc<dyn UserRepository>,
}

impl LoginEmailHandler {
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }

    pub async fn execute(&self, cmd: LoginEmailCommand) -> Result<LoginResult, LoginError> {
        let email = normalize_email(&cmd.email)?;

        // Placeholder until repository and JWT signing are wired
        Ok(LoginResult {
            id: String::new(),
            email,
            username: String::new(),
        })
    }
}
