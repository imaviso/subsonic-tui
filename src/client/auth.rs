//! Authentication utilities for the OpenSubsonic API.

use md5::{Digest, Md5};
use rand::Rng;

/// Authentication credentials for API requests.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Auth {
    /// Token-based authentication (recommended for API 1.13.0+)
    Token {
        username: String,
        token: String,
        salt: String,
    },
    /// API key authentication (OpenSubsonic extension)
    ApiKey { api_key: String },
    /// Legacy password authentication (API 1.12.0 and earlier)
    Password { username: String, password: String },
}

impl Auth {
    /// Create token-based authentication from username and password.
    pub fn from_password(username: impl Into<String>, password: &str) -> Self {
        let username = username.into();
        let salt = generate_salt();
        let token = generate_token(password, &salt);

        Self::Token {
            username,
            token,
            salt,
        }
    }

    /// Create API key authentication.
    pub fn from_api_key(api_key: impl Into<String>) -> Self {
        Self::ApiKey {
            api_key: api_key.into(),
        }
    }

    /// Create legacy password authentication.
    #[allow(dead_code)]
    pub fn from_legacy_password(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Password {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Get query parameters for authentication.
    pub fn query_params(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Token {
                username,
                token,
                salt,
            } => vec![
                ("u", username.clone()),
                ("t", token.clone()),
                ("s", salt.clone()),
            ],
            Self::ApiKey { api_key } => vec![("apiKey", api_key.clone())],
            Self::Password { username, password } => {
                vec![("u", username.clone()), ("p", password.clone())]
            }
        }
    }

    /// Regenerate the salt and token for token-based auth.
    /// This should be called before each request for maximum security.
    #[allow(dead_code)]
    pub fn regenerate(&mut self, password: &str) {
        if let Self::Token {
            salt,
            token,
            username,
            ..
        } = self
        {
            *salt = generate_salt();
            *token = generate_token(password, salt);
            let _ = username; // Keep username unchanged
        }
    }
}

/// Generate a random salt string.
fn generate_salt() -> String {
    let mut rng = rand::thread_rng();
    let salt: String = (0..16)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    salt
}

/// Generate authentication token: md5(password + salt).
fn generate_token(password: &str, salt: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(password.as_bytes());
    hasher.update(salt.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation() {
        // Example from OpenSubsonic docs:
        // password = "sesame", salt = "c19b2d"
        // token = md5("sesamec19b2d") = "26719a1196d2a940705a59634eb18eab"
        let token = generate_token("sesame", "c19b2d");
        assert_eq!(token, "26719a1196d2a940705a59634eb18eab");
    }

    #[test]
    fn test_salt_length() {
        let salt = generate_salt();
        assert_eq!(salt.len(), 16);
    }

    #[test]
    fn test_auth_from_password() {
        let auth = Auth::from_password("testuser", "testpass");
        let params = auth.query_params();

        assert_eq!(params.len(), 3);
        assert_eq!(params[0].0, "u");
        assert_eq!(params[0].1, "testuser");
        assert_eq!(params[1].0, "t");
        assert_eq!(params[2].0, "s");
    }

    #[test]
    fn test_auth_api_key() {
        let auth = Auth::from_api_key("my-api-key");
        let params = auth.query_params();

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, "apiKey");
        assert_eq!(params[0].1, "my-api-key");
    }
}
