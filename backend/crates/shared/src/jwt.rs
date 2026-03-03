//! Create and verify JWTs using a shared secret (HS256).

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JwtError {
    #[error("JWT error: {0}")]
    Token(#[from] jsonwebtoken::errors::Error),
}

/// Create a signed JWT from the given claims.
/// Uses HS256 and the provided secret.
pub fn create<C: Serialize>(claims: &C, secret: &[u8]) -> Result<String, JwtError> {
    let token = encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret),
    )?;
    Ok(token)
}

/// Verify a JWT and decode its claims.
/// Uses HS256 and the provided secret. Validates expiration by default.
pub fn verify<C: DeserializeOwned>(token: &str, secret: &[u8]) -> Result<C, JwtError> {
    let token_data = decode::<C>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}
