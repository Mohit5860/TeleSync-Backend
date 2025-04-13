use mongodb::{
    Client, Collection,
    bson::{doc, oid::ObjectId},
    error::Result,
};
use std::{env, sync::Arc};

use crate::models::{participant_model::Participant, room_model::Room, user_model::User};

pub struct Database {
    pub user: Collection<User>,
    pub room: Collection<Room>,
    pub participant: Collection<Participant>,
}

impl Database {
    pub async fn init() -> Result<Self> {
        let db_url = env::var("MONGODB_URI").expect("❌ MONGODB_URI not found in .env");
        let client = Client::with_uri_str(&db_url).await?;

        let db_name = env::var("DB_NAME").unwrap_or_else(|_| "my_database".to_string());
        let db = client.database(&db_name);

        let user: Collection<User> = db.collection("users");
        let room: Collection<Room> = db.collection("rooms");
        let participant: Collection<Participant> = db.collection("participants");

        Ok(Database {
            user,
            room,
            participant,
        })
    }

    pub async fn get_user_by_id(
        db: Arc<Database>,
        user_id: ObjectId,
    ) -> mongodb::error::Result<Option<User>> {
        let filter = doc! {"_id" : user_id};
        let user = db.user.find_one(filter, None).await?;

        Ok(user)
    }

    pub async fn create_room(
        db: Arc<Database>,
        host_id: ObjectId,
        code: String,
    ) -> mongodb::error::Result<()> {
        let new_room = Room {
            _id: Some(ObjectId::new()),
            host_id,
            code,
            participants_id: vec![],
        };

        db.room.insert_one(new_room, None).await?;
        Ok(())
    }

    pub async fn get_room_by_code(
        db: Arc<Database>,
        room_code: &str,
    ) -> mongodb::error::Result<Option<Room>> {
        let filter = doc! { "code": room_code };
        let room = db.room.find_one(filter, None).await?;

        Ok(room)
    }

    pub async fn add_participant_to_room(
        db: Arc<Database>,
        room_code: &str,
        user_id: ObjectId,
    ) -> mongodb::error::Result<()> {
        let filter = doc! { "code": room_code };
        let update = doc! { "$addToSet": { "participants_id": user_id } };

        db.room.update_one(filter, update, None).await?;
        Ok(())
    }

    pub async fn remove_participant_from_room(
        db: Arc<Database>,
        room_code: &str,
        user_id: ObjectId,
    ) -> mongodb::error::Result<()> {
        let filter = doc! {"code": room_code};
        let update = doc! {
            "$pull": { "participants_id": user_id }
        };

        db.room.update_one(filter, update, None).await?;
        Ok(())
    }

    pub async fn update_host_id(
        db: Arc<Database>,
        room_code: &str,
        new_host_id: ObjectId,
    ) -> mongodb::error::Result<()> {
        let filter = doc! { "code": room_code };
        let update = doc! {
            "$set": { "host_id": new_host_id }
        };

        db.room.update_one(filter, update, None).await?;

        Ok(())
    }

    pub async fn delete_room(db: Arc<Database>, room_code: &str) -> mongodb::error::Result<()> {
        let filter = doc! { "code": room_code };
        db.room.delete_one(filter, None).await?;
        Ok(())
    }

    pub async fn add_participant(
        db: Arc<Database>,
        room_code: String,
        user_id: ObjectId,
    ) -> mongodb::error::Result<()> {
        let new_participant = Participant {
            _id: Some(ObjectId::new()),
            user_id,
            room_code,
        };

        db.participant.insert_one(new_participant, None).await?;

        Ok(())
    }
}
