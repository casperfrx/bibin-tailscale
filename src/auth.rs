use rocket::form;
use rocket::http::Status;
use rocket::outcome::Outcome;
use rocket::request::{self, FromRequest, Request};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub struct AuthKey(String);

impl<'v> rocket::form::FromFormField<'v> for AuthKey {
    fn from_value(value: form::ValueField) -> form::Result<'v, Self> {
        Ok(AuthKey(String::from(value.value)))
    }
}

#[derive(Debug)]
pub enum AuthError {
    Missing,
    BadCount,
    UnsupportedAuth,
    InvalidData,
}

impl AuthKey {
    pub fn is_valid(&self, password: &AuthKey) -> bool {
        password.0 == self.0
    }
}

impl std::convert::From<base64::DecodeError> for AuthError {
    fn from(error: base64::DecodeError) -> AuthError {
        warn!("[AUTH] Invalid Base64 value: {}", error);
        Self::InvalidData
    }
}

impl std::convert::From<std::str::Utf8Error> for AuthError {
    fn from(error: std::str::Utf8Error) -> AuthError {
        warn!("[AUTH] Invalid password String: {}", error);
        Self::InvalidData
    }
}

fn auth_from_api_header(request: &'_ Request<'_>) -> Result<Option<AuthKey>, AuthError> {
    let api_keys: Vec<_> = request.headers().get("X-API-Key").collect();
    match api_keys.len() {
        0 => Ok(None),
        1 => Ok(Some(AuthKey(api_keys[0].to_string()))),
        _ => Err(AuthError::BadCount),
    }
}

fn auth_from_auth_header(request: &'_ Request<'_>) -> Result<Option<AuthKey>, AuthError> {
    let basic_tokens: Vec<_> = request.headers().get("Authorization").collect();
    match basic_tokens.len() {
        0 => Ok(None),
        1 => {
            let basic_token = basic_tokens[0];
            if !basic_token.starts_with("Basic ") {
                Err(AuthError::UnsupportedAuth)
            } else {
                let token: Vec<&str> = basic_token.splitn(2, ' ').collect();
                let decoded_token = &base64::decode(token[1])?;
                let decoded = std::str::from_utf8(decoded_token)?;
                if !decoded.contains(':') {
                    Err(AuthError::InvalidData)
                } else {
                    let decoded_token: Vec<&str> = decoded.splitn(2, ':').collect();
                    Ok(Some(AuthKey(decoded_token[1].to_string())))
                }
            }
        }
        _ => Err(AuthError::BadCount),
    }
}

#[rocket::async_trait]
impl<'a> FromRequest<'a> for AuthKey {
    type Error = AuthError;

    async fn from_request(request: &'a Request<'_>) -> request::Outcome<Self, Self::Error> {
        match auth_from_api_header(request) {
            Ok(Some(x)) => return Outcome::Success(x),
            Err(x) => return Outcome::Failure((Status::Unauthorized, x)),
            _ => {}
        };
        debug!("[AUTH] No API Header found");

        match auth_from_auth_header(request) {
            Ok(Some(x)) => return Outcome::Success(x),
            Err(x) => return Outcome::Failure((Status::Unauthorized, x)),
            _ => {}
        };
        debug!("[AUTH] No Authorization Header found");

        Outcome::Failure((Status::Unauthorized, AuthError::Missing))
    }
}
