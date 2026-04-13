//! Create and verify JWTs using a shared secret (HS256).

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

/// Errors from JWT creation and verification.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestClaims {
        sub: String,
        exp: i64,
    }

    fn future_exp() -> i64 {
        chrono::Utc::now().timestamp() + 3600
    }

    fn past_exp() -> i64 {
        chrono::Utc::now().timestamp() - 3600
    }

    #[test]
    fn create_and_verify_round_trip() {
        let secret = b"test-secret-key";
        let claims = TestClaims {
            sub: "user-123".into(),
            exp: future_exp(),
        };

        let token = create(&claims, secret).unwrap();
        let decoded: TestClaims = verify(&token, secret).unwrap();
        assert_eq!(decoded.sub, "user-123");
    }

    #[test]
    fn verify_rejects_expired_token() {
        let secret = b"test-secret-key";
        let claims = TestClaims {
            sub: "user-123".into(),
            exp: past_exp(),
        };

        let token = create(&claims, secret).unwrap();
        let result = verify::<TestClaims>(&token, secret);
        assert!(result.is_err());
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let claims = TestClaims {
            sub: "user-123".into(),
            exp: future_exp(),
        };

        let token = create(&claims, b"secret-a").unwrap();
        let result = verify::<TestClaims>(&token, b"secret-b");
        assert!(result.is_err());
    }

    #[test]
    fn verify_rejects_garbage_token() {
        let result = verify::<TestClaims>("not.a.jwt", b"secret");
        assert!(result.is_err());
    }
}
