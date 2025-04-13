use mongodb::bson::oid::ObjectId;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Participant {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")] 
    pub _id: Option<ObjectId>,

    pub user_id: ObjectId,
    pub room_code: String,
}