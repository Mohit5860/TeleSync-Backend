use mongodb::bson::oid::ObjectId;
use serde::{Serialize, Deserialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")] 
    pub _id: Option<ObjectId>,

    pub username: String,
    pub email: String,
    pub password: String,

    // #[serde(skip_serializing_if = "Option::is_none")] 
    // pub refresh_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterUser {
    pub username: String,
    pub email: String,
    pub password: String
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginUser {
    pub email: String,
    pub password: String,
}
