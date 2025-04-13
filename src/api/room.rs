use axum::{
    routing::post, Router, extract::State, response::{IntoResponse, Json}, http:: StatusCode,
};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use rand::Rng;

use crate::{
    db::connection::Database,
    utils::jwt::verify_access_token, SharedState,
};

#[derive(Debug, Serialize, Deserialize)]
struct CreateRequest {
    access_token: String
}

fn generate_code() -> String {
    let code: u32 = rand::thread_rng().gen_range(100000..999999);
    code.to_string()
}

async fn create_room(
    State(state): State<SharedState>, 
    Json(payload): Json<CreateRequest>
) -> Result<impl IntoResponse, StatusCode> {

    let db = state.db.clone();

    let claim = match verify_access_token(&payload.access_token) {
        Ok(claim) => claim,
        Err(err) => {
            eprintln!("❌ JWT verification failed: {:?}", err);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let host_id = match ObjectId::parse_str(&claim.sub) {
        Ok(oid) => oid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let code = generate_code();

    match Database::create_room(db.clone(), host_id, code.clone()).await {
        Ok(_) => {
            // Add the host as a participant
            match Database::add_participant(db.clone(), code.clone(), host_id).await {
                Ok(_) => Ok((
                    StatusCode::CREATED, 
                    Json(json!({
                        "success": true,
                        "message": "Room created successfully",
                        "code": code
                    }))
                )),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}


pub fn room_router() -> Router<SharedState> {
    Router::new()
        .route("/create", post(create_room))
}