use mongodb::bson::oid::ObjectId;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Room {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")] 
    pub _id: Option<ObjectId>,

    pub host_id: ObjectId,
    pub code: String,

    #[serde(default)]
    pub participants_id: Vec<ObjectId>,
}
