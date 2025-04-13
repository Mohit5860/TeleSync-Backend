use axum::{
    routing::post, Router, extract::State, response::{IntoResponse, Json}, http:: StatusCode,
};
use mongodb::{Collection, bson::{doc, oid::ObjectId}};
use serde_json::json;

use crate::{
     models::user_model::{LoginUser, RegisterUser, User}, utils::{bcrypt::{hash_password, verify_password}, jwt::{generate_access_token, generate_refresh_token}}, SharedState
};

async fn register(
    State(state): State<SharedState>, 
    Json(payload): Json<RegisterUser>
) -> Result<impl IntoResponse, StatusCode> {
    let db = state.db;
    let user_collection: &Collection<User> = &db.user;

    if user_collection
        .find_one(doc! {"email": &payload.email}, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .is_some()
    {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": "User already exists with this email." }))
        ));
    }

    let hashed_password = hash_password(&payload.password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let new_user = User {
        _id: Some(ObjectId::new()),
        username: payload.username.clone(),
        email: payload.email.clone(),
        password: hashed_password,
    };

    user_collection.insert_one(&new_user, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "message": "User registered successfully." }))
    ))
}

async fn login(
    State(state): State<SharedState>,
    Json(payload): Json<LoginUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let db = state.db.clone();
    let user_collection: &Collection<User> = &db.user;

    let user = user_collection
        .find_one(doc! {"email": &payload.email}, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if let Ok(false) | Err(_) = verify_password(&payload.password, &user.password) {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "message": "Incorrect password." })),
        ));
    }

    let user_id = user._id.expect("User id not found in DB.").to_hex();
    let access_token = generate_access_token(&user_id, &user.username, &user.email);
    let refresh_token = generate_refresh_token(&user_id);

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "User logged in successfully.",
            "access_token": access_token,
            "refresh_token": refresh_token
        }))
    ))
}

pub fn auth_router() -> Router<SharedState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
}
