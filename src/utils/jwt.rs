use serde::{Serialize, Deserialize};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey, errors::Error};
use chrono::{Utc, Duration};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: String,
    pub username: String,
    pub email: String,
    pub exp: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: String,
    pub exp: usize,
}

pub fn generate_access_token(user_id: &str, username: &str, email: &str) -> String {
    let expiration = Utc::now() + Duration::hours(2);
    let access_claims = AccessClaims {
        sub: user_id.to_owned(),
        username: username.to_owned(),
        email: email.to_owned(),
        exp: expiration.timestamp() as usize,
    };
    let secret = env::var("ACCESS_TOKEN_SECRET").expect("Access token secret not found in .env");

    encode(
        &Header::default(),
        &access_claims,
        &EncodingKey::from_secret(secret.as_ref()),
    ).expect("Failed to generate access token.")
}

pub fn verify_access_token(token: &str) -> Result<AccessClaims, Error> {
    let secret = env::var("ACCESS_TOKEN_SECRET")
        .expect("❌ Access token secret not found in .env");

    let token_data = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;
    
    Ok(token_data.claims)
}

pub fn generate_refresh_token(user_id: &str) -> String {
    let expiration = Utc::now() + Duration::days(7);
    let refresh_claims = RefreshClaims {
        sub: user_id.to_owned(),
        exp: expiration.timestamp() as usize,
    };
    let secret = env::var("REFRESH_TOKEN_SECRET").expect("Refresh token secret not found in .env");

    encode(
        &Header::default(),
        &refresh_claims,
        &EncodingKey::from_secret(secret.as_ref()),
    ).expect("Failed to generate refresh token.")
}

pub fn verify_refresh_token(token: &str) -> Option<RefreshClaims> {
    let secret = env::var("REFRESH_TOKEN_SECRET").expect("Refresh token secret not found in .env");
    decode::<RefreshClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )
    .ok()
    .map(|data| data.claims)
}
