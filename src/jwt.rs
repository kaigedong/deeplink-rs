use axum::async_trait;
use axum::extract::{FromRequestParts, TypedHeader};
use axum::headers::{authorization::Bearer, Authorization};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use jsonwebtoken as jwt;
use jwt::Validation;
use serde::{Deserialize, Serialize};

const SECRET: &[u8] = b"deadbeef";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: String,
    pub device_id: String,
    // Required. Expiration time (as UTC timestamp)
    pub exp: usize,
}

pub fn new_token(user_id: String, device_id: String) -> String {
    // skip login validation: 如果用户正确传入email & passed，将为该用户生成一个Token
    let claims = Claims {
        user_id,
        device_id,
        exp: get_epoch() + 14 * 24 * 60 * 60, // 14天后过期
    };
    let key = jwt::EncodingKey::from_secret(SECRET);
    let token = jwt::encode(&jwt::Header::default(), &claims, &key).unwrap();
    token
}

fn get_epoch() -> usize {
    use std::time::SystemTime;
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as usize
}

#[async_trait]
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = HttpError;
    async fn from_request_parts(
        parts: &mut Parts,
        // req: &mut RequestParts<B>,
        state: &S,
    ) -> anyhow::Result<Self, Self::Rejection> {
        // 要求Axum使用features = ["headers"]
        // 拿到bear token
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| HttpError::Auth)?;
        let key = jwt::DecodingKey::from_secret(SECRET);
        // Decode bear token
        let token = jwt::decode::<Claims>(bearer.token(), &key, &Validation::default())
            .map_err(|_e| HttpError::Auth)?;
        Ok(token.claims)
    }
}

#[derive(Debug)]
pub enum HttpError {
    Auth,
    Internal,
}

impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        let (code, msg) = match self {
            HttpError::Auth => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            HttpError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
        };
        // Axum已经实现的如果tupe每个字段都实现了into_response，那这个tupe也会实现into_response
        (code, msg).into_response()
    }
}
