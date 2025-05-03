use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use futures_util::SinkExt as FuturesSinkExt;
use futures_util::{
    StreamExt,
    stream::{SplitSink, SplitStream},
};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task;
use uuid::Uuid;

use crate::{
    SharedState,
    db::connection::Database,
    models::{room_model::Room, user_model::User},
    utils::jwt::{AccessClaims, verify_access_token},
};

#[derive(Clone)]
pub struct AppState {
    pub user_sockets: Arc<Mutex<HashMap<ObjectId, Uuid>>>,
    pub sockets: Arc<Mutex<HashMap<Uuid, Arc<Mutex<SplitSink<WebSocket, Message>>>>>>,
}

#[derive(Deserialize)]
struct JoinRoomData {
    access_token: String,
    code: String,
}

#[derive(Serialize)]
struct JoinRoomResponse {
    message_type: String,
    user_id: ObjectId,
    username: String,
}

#[derive(Deserialize, Serialize, Clone)]
struct Participant {
    username: String,
    id: ObjectId,
    video: bool,
}

#[derive(Deserialize, Serialize, Clone)]
struct Host {
    username: String,
    id: ObjectId,
    video: bool,
    screen: bool,
}

#[derive(Deserialize, Serialize)]
struct RequestAcceptedData {
    username: String,
    user_id: ObjectId,
    participants: Vec<Participant>,
    code: String,
    host: Host,
}

#[derive(Serialize)]
struct RequestAcceptedResponse {
    message_type: String,
    username: String,
    user_id: ObjectId,
    participants: Vec<Participant>,
    host: Host,
}

#[derive(Serialize)]
struct RequestAcceptedResponseTOParticipants {
    message_type: String,
    username: String,
    user_id: ObjectId,
    participant: ObjectId,
    host: Host,
}

#[derive(Deserialize)]
struct RtcConnectionData {
    item: serde_json::Value,
    to: ObjectId,
    user_id: ObjectId,
}

#[derive(Serialize)]
struct RtcConnectionResponse {
    message_type: String,
    item: serde_json::Value,
    from: ObjectId,
    user_id: ObjectId,
}

#[derive(Deserialize)]
struct MouseMoveData {
    x: f64,
    y: f64,
    to: ObjectId,
}

#[derive(Serialize)]
struct MouseMoveResponse {
    message_type: String,
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
struct KeyPressData {
    key: String,
    to: ObjectId,
}

#[derive(Serialize)]
struct KeyPressResponse {
    message_type: String,
    key: String,
}

#[derive(Deserialize)]
struct MouseClickData {
    to: ObjectId,
}

#[derive(Serialize)]
struct MouseClickResponse {
    message_type: String,
}

#[derive(Deserialize)]
struct MessageData {
    message: String,
    username: String,
    id: ObjectId,
    code: String,
}

#[derive(Serialize)]
struct MessageResponse {
    message_type: String,
    message: String,
    username: String,
    id: ObjectId,
}

#[derive(Deserialize)]
struct VideoData {
    user_id: ObjectId,
    code: String,
    host: bool,
}

#[derive(Serialize)]
struct VideoResponse {
    message_type: String,
    user_id: ObjectId,
    host: bool,
}

#[derive(Deserialize)]
struct LeaveRoomData {
    code: String,
    user_id: ObjectId,
}

#[derive(Serialize)]
struct HostLeftresponse {
    message_type: String,
    host: ObjectId,
    username: String,
}

#[derive(Serialize)]
struct ParticipantLeft {
    message_type: String,
    user: ObjectId,
}

#[derive(Deserialize)]
struct RequestAccessData {
    to: ObjectId,
    from: ObjectId,
    username: String,
}

#[derive(Serialize)]
struct RequestAccessResponse {
    message_type: String,
    user_id: ObjectId,
    username: String,
}

#[derive(Deserialize)]
struct AccessData {
    code: String,
    user_id: ObjectId,
    username: String,
}

#[derive(Serialize)]
struct AccessResponse {
    message_type: String,
    user_id: ObjectId,
    username: String,
}

pub async fn handler(ws: WebSocketUpgrade, State(state): State<SharedState>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: SharedState) {
    let (sender, receiver) = socket.split();
    let db = state.db.clone();
    let ws_state = state.ws_state.clone();

    let socket_id = Uuid::new_v4();

    {
        let mut sockets = ws_state.sockets.lock().await;
        sockets.insert(socket_id, Arc::new(Mutex::new(sender)));
    }

    task::spawn(handle_rooms(receiver, socket_id, db, ws_state));
}

async fn handle_rooms(
    mut receiver: SplitStream<WebSocket>,
    socket_id: Uuid,
    db: Arc<Database>,
    ws_state: Arc<AppState>,
) {
    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(message_type) = json["type"].as_str() {
                        match message_type {
                            "join-room" => {
                                let data: JoinRoomData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => continue,
                                    };

                                let token = data.access_token;

                                let claim: AccessClaims = match verify_access_token(&token) {
                                    Ok(claim) => claim,
                                    Err(_) => continue,
                                };

                                let oid: ObjectId = match ObjectId::parse_str(&claim.sub) {
                                    Ok(id) => id,
                                    Err(_) => continue,
                                };

                                {
                                    let mut user_sockets = ws_state.user_sockets.lock().await;
                                    user_sockets.insert(oid, socket_id);
                                }

                                let room: Room = match Database::get_room_by_code(
                                    db.clone(),
                                    &data.code,
                                )
                                .await
                                {
                                    Ok(Some(room)) => room,
                                    _ => continue,
                                };

                                let user: User =
                                    match Database::get_user_by_id(db.clone(), oid).await {
                                        Ok(Some(user)) => user,
                                        _ => continue,
                                    };

                                let response: JoinRoomResponse;
                                let host_id = if oid == room.host_id {
                                    response = JoinRoomResponse {
                                        message_type: "host-joined".to_string(),
                                        user_id: oid,
                                        username: user.username,
                                    };
                                    oid
                                } else {
                                    let host =
                                        match Database::get_user_by_id(db.clone(), room.host_id)
                                            .await
                                        {
                                            Ok(Some(host)) => host,
                                            _ => continue,
                                        };

                                    let host_id = match host._id {
                                        Some(id) => id,
                                        None => continue,
                                    };

                                    response = JoinRoomResponse {
                                        message_type: "join-request".to_string(),
                                        user_id: oid,
                                        username: user.username,
                                    };
                                    host_id
                                };

                                let response_text = serde_json::to_string(&response).unwrap();

                                let sender_id = {
                                    let user_sockets = ws_state.user_sockets.lock().await;
                                    user_sockets.get(&host_id).cloned()
                                };

                                if let Some(sender_id) = sender_id {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(&sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }

                                println!("data send to user");
                            }
                            "request-accepted" => {
                                println!("request recieved");
                                let data: RequestAcceptedData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => continue,
                                    };

                                if let Err(_) = Database::add_participant_to_room(
                                    db.clone(),
                                    &data.code,
                                    data.user_id,
                                )
                                .await
                                {
                                    continue;
                                }

                                if let Err(_) = Database::add_participant(
                                    db.clone(),
                                    data.code.clone(),
                                    data.user_id,
                                )
                                .await
                                {
                                    continue;
                                }

                                let user_sockets = ws_state.user_sockets.lock().await.clone();

                                for participant in &data.participants {
                                    let response_to_participants =
                                        RequestAcceptedResponseTOParticipants {
                                            message_type: "new-participant".to_string(),
                                            user_id: data.user_id,
                                            username: data.username.clone(),
                                            participant: participant.id,
                                            host: data.host.clone(),
                                        };
                                    let response_to_participant_text =
                                        serde_json::to_string(&response_to_participants).unwrap();
                                    if let Some(sender_id) = user_sockets.get(&participant.id) {
                                        let sender_arc = {
                                            let sockets = ws_state.sockets.lock().await;
                                            sockets.get(sender_id).cloned()
                                        };

                                        if let Some(sender_arc) = sender_arc {
                                            let mut sender = sender_arc.lock().await;
                                            if let Err(err) = sender
                                                .send(Message::Text(
                                                    response_to_participant_text.clone().into(),
                                                ))
                                                .await
                                            {
                                                eprintln!(
                                                    "Failed to send message to user {}: {}",
                                                    sender_id, err
                                                );
                                            }
                                        }
                                    }
                                }
                                println!("response sent to participants");

                                let response_to_host = RequestAcceptedResponseTOParticipants {
                                    message_type: "new-participant".to_string(),
                                    user_id: data.user_id,
                                    username: data.username.clone(),
                                    participant: data.host.id,
                                    host: data.host.clone(),
                                };
                                let response_to_host_text =
                                    serde_json::to_string(&response_to_host).unwrap();

                                if let Some(sender_id) = user_sockets.get(&data.host.id) {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) = sender
                                            .send(Message::Text(
                                                response_to_host_text.clone().into(),
                                            ))
                                            .await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }

                                let response = RequestAcceptedResponse {
                                    message_type: "participant-joined".to_string(),
                                    user_id: data.user_id,
                                    username: data.username.clone(),
                                    participants: data.participants.clone(),
                                    host: data.host.clone(),
                                };
                                let response_text = serde_json::to_string(&response).unwrap();

                                if let Some(sender_id) = user_sockets.get(&data.user_id) {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }

                                println!("response sent")
                            }
                            "offer" | "answer" | "ice-candidate" => {
                                let data: RtcConnectionData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let response = RtcConnectionResponse {
                                    message_type: message_type.to_string(),
                                    item: data.item,
                                    from: data.user_id,
                                    user_id: data.to,
                                };
                                let response_text = serde_json::to_string(&response).unwrap();

                                let sender_id = {
                                    let user_sockets = ws_state.user_sockets.lock().await;
                                    user_sockets.get(&data.to).cloned()
                                };

                                if let Some(sender_id) = sender_id {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(&sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }
                            }

                            "mouse-move" => {
                                let data: MouseMoveData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let response: MouseMoveResponse = MouseMoveResponse {
                                    message_type: "mouse-move".to_string(),
                                    x: data.x,
                                    y: data.y,
                                };

                                let response_text = serde_json::to_string(&response).unwrap();

                                let sender_id = {
                                    let user_sockets = ws_state.user_sockets.lock().await;
                                    user_sockets.get(&data.to).cloned()
                                };

                                if let Some(sender_id) = sender_id {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(&sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }
                            }

                            "key-press" => {
                                let data: KeyPressData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let response: KeyPressResponse = KeyPressResponse {
                                    message_type: "key-press".to_string(),
                                    key: data.key,
                                };

                                let response_text = serde_json::to_string(&response).unwrap();

                                let sender_id = {
                                    let user_sockets = ws_state.user_sockets.lock().await;
                                    user_sockets.get(&data.to).cloned()
                                };

                                if let Some(sender_id) = sender_id {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(&sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }
                            }

                            "mouse-click" => {
                                let data: MouseClickData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let response: MouseClickResponse = MouseClickResponse {
                                    message_type: "mouse-click".to_string(),
                                };

                                let response_text = serde_json::to_string(&response).unwrap();

                                let sender_id = {
                                    let user_sockets = ws_state.user_sockets.lock().await;
                                    user_sockets.get(&data.to).cloned()
                                };

                                if let Some(sender_id) = sender_id {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(&sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }
                            }

                            "message" => {
                                let data: MessageData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let room: Room = match Database::get_room_by_code(
                                    db.clone(),
                                    &data.code,
                                )
                                .await
                                {
                                    Ok(Some(room)) => room,
                                    _ => continue,
                                };

                                let response: MessageResponse = MessageResponse {
                                    message_type: "message".to_string(),
                                    message: data.message,
                                    username: data.username,
                                    id: data.id,
                                };

                                let response_text = serde_json::to_string(&response).unwrap();

                                let user_sockets = ws_state.user_sockets.lock().await.clone();

                                for participant in &room.participants_id {
                                    if let Some(sender_id) = user_sockets.get(&participant) {
                                        let sender_arc = {
                                            let sockets = ws_state.sockets.lock().await;
                                            sockets.get(sender_id).cloned()
                                        };

                                        if let Some(sender_arc) = sender_arc {
                                            let mut sender = sender_arc.lock().await;
                                            if let Err(err) = sender
                                                .send(Message::Text(response_text.clone().into()))
                                                .await
                                            {
                                                eprintln!(
                                                    "Failed to send message to user {}: {}",
                                                    sender_id, err
                                                );
                                            }
                                        }
                                    }
                                }

                                if let Some(sender_id) = user_sockets.get(&room.host_id) {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }
                            }
                            "screen-sharing-started" | "screen-sharing-stopped" => {
                                let data: VideoData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let room: Room = match Database::get_room_by_code(
                                    db.clone(),
                                    &data.code,
                                )
                                .await
                                {
                                    Ok(Some(room)) => room,
                                    _ => continue,
                                };
                                let response: VideoResponse = VideoResponse {
                                    message_type: message_type.to_string(),
                                    user_id: data.user_id,
                                    host: data.host,
                                };

                                let response_text = serde_json::to_string(&response).unwrap();

                                let user_sockets = ws_state.user_sockets.lock().await.clone();

                                for participant in &room.participants_id {
                                    if let Some(sender_id) = user_sockets.get(&participant) {
                                        let sender_arc = {
                                            let sockets = ws_state.sockets.lock().await;
                                            sockets.get(sender_id).cloned()
                                        };

                                        if let Some(sender_arc) = sender_arc {
                                            let mut sender = sender_arc.lock().await;
                                            if let Err(err) = sender
                                                .send(Message::Text(response_text.clone().into()))
                                                .await
                                            {
                                                eprintln!(
                                                    "Failed to send message to user {}: {}",
                                                    sender_id, err
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            "video-started" | "video-stopped" => {
                                let data: VideoData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let room: Room = match Database::get_room_by_code(
                                    db.clone(),
                                    &data.code,
                                )
                                .await
                                {
                                    Ok(Some(room)) => room,
                                    _ => continue,
                                };
                                let response: VideoResponse = VideoResponse {
                                    message_type: message_type.to_string(),
                                    user_id: data.user_id,
                                    host: data.host,
                                };

                                let response_text = serde_json::to_string(&response).unwrap();

                                let user_sockets = ws_state.user_sockets.lock().await.clone();

                                for participant in &room.participants_id {
                                    if let Some(sender_id) = user_sockets.get(&participant) {
                                        let sender_arc = {
                                            let sockets = ws_state.sockets.lock().await;
                                            sockets.get(sender_id).cloned()
                                        };

                                        if let Some(sender_arc) = sender_arc {
                                            let mut sender = sender_arc.lock().await;
                                            if let Err(err) = sender
                                                .send(Message::Text(response_text.clone().into()))
                                                .await
                                            {
                                                eprintln!(
                                                    "Failed to send message to user {}: {}",
                                                    sender_id, err
                                                );
                                            }
                                        }
                                    }
                                }

                                if let Some(sender_id) = user_sockets.get(&room.host_id) {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }
                            }

                            "leave-room" => {
                                let data: LeaveRoomData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let room: Room = match Database::get_room_by_code(
                                    db.clone(),
                                    &data.code,
                                )
                                .await
                                {
                                    Ok(Some(room)) => room,
                                    _ => continue,
                                };

                                let response_text: String;

                                if data.user_id == room.host_id {
                                    if room.participants_id.len() == 0 {
                                        match Database::delete_room(db.clone(), &data.code).await {
                                            Ok(_) => println!("Room deleted"),
                                            Err(_) => continue,
                                        };
                                        continue;
                                    }
                                    let user_id = room.participants_id[0];
                                    match Database::remove_participant_from_room(
                                        db.clone(),
                                        &data.code,
                                        user_id,
                                    )
                                    .await
                                    {
                                        Ok(_) => println!("Participant removed"),
                                        Err(_) => continue,
                                    };
                                    match Database::update_host_id(db.clone(), &data.code, user_id)
                                        .await
                                    {
                                        Ok(_) => println!("Host changed"),
                                        Err(_) => continue,
                                    };
                                    let user: User =
                                        match Database::get_user_by_id(db.clone(), user_id).await {
                                            Ok(Some(user)) => user,
                                            _ => continue,
                                        };
                                    let response = HostLeftresponse {
                                        message_type: "host-left".to_string(),
                                        host: user_id,
                                        username: user.username,
                                    };

                                    response_text = serde_json::to_string(&response).unwrap();
                                } else {
                                    match Database::remove_participant_from_room(
                                        db.clone(),
                                        &data.code,
                                        data.user_id,
                                    )
                                    .await
                                    {
                                        Ok(_) => println!("Participant removed"),
                                        Err(_) => continue,
                                    };

                                    let response = ParticipantLeft {
                                        message_type: "participant-left".to_string(),
                                        user: data.user_id,
                                    };

                                    response_text = serde_json::to_string(&response).unwrap();
                                }

                                let user_sockets = ws_state.user_sockets.lock().await.clone();

                                for participant in &room.participants_id {
                                    if let Some(sender_id) = user_sockets.get(&participant) {
                                        let sender_arc = {
                                            let sockets = ws_state.sockets.lock().await;
                                            sockets.get(sender_id).cloned()
                                        };

                                        if let Some(sender_arc) = sender_arc {
                                            let mut sender = sender_arc.lock().await;
                                            if let Err(err) = sender
                                                .send(Message::Text(response_text.clone().into()))
                                                .await
                                            {
                                                eprintln!(
                                                    "Failed to send message to user {}: {}",
                                                    sender_id, err
                                                );
                                            }
                                        }
                                    }
                                }

                                if let Some(sender_id) = user_sockets.get(&room.host_id) {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }
                            }
                            "request-access" => {
                                let data: RequestAccessData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let response: RequestAccessResponse = RequestAccessResponse {
                                    message_type: "request-access".to_string(),
                                    user_id: data.from,
                                    username: data.username,
                                };

                                let response_text = serde_json::to_string(&response).unwrap();

                                let user_sockets = ws_state.user_sockets.lock().await.clone();

                                if let Some(sender_id) = user_sockets.get(&data.to) {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }
                            }

                            "allowed-access" => {
                                let data: AccessData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let room: Room = match Database::get_room_by_code(
                                    db.clone(),
                                    &data.code,
                                )
                                .await
                                {
                                    Ok(Some(room)) => room,
                                    _ => continue,
                                };

                                let response: AccessResponse = AccessResponse {
                                    message_type: "allowed-access".to_string(),
                                    user_id: data.user_id,
                                    username: data.username,
                                };
                                let response_text = serde_json::to_string(&response).unwrap();
                                let user_sockets = ws_state.user_sockets.lock().await.clone();

                                for participant in &room.participants_id {
                                    if let Some(sender_id) = user_sockets.get(&participant) {
                                        let sender_arc = {
                                            let sockets = ws_state.sockets.lock().await;
                                            sockets.get(sender_id).cloned()
                                        };

                                        if let Some(sender_arc) = sender_arc {
                                            let mut sender = sender_arc.lock().await;
                                            if let Err(err) = sender
                                                .send(Message::Text(response_text.clone().into()))
                                                .await
                                            {
                                                eprintln!(
                                                    "Failed to send message to user {}: {}",
                                                    sender_id, err
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            "rejected-access" => {
                                let data: AccessData =
                                    match serde_json::from_value(json["data"].clone()) {
                                        Ok(d) => d,
                                        Err(_) => {
                                            println!("err");
                                            continue;
                                        }
                                    };

                                let response: AccessResponse = AccessResponse {
                                    message_type: "allowed-access".to_string(),
                                    user_id: data.user_id,
                                    username: data.username,
                                };
                                let response_text = serde_json::to_string(&response).unwrap();

                                let user_sockets = ws_state.user_sockets.lock().await.clone();

                                if let Some(sender_id) = user_sockets.get(&data.user_id) {
                                    let sender_arc = {
                                        let sockets = ws_state.sockets.lock().await;
                                        sockets.get(sender_id).cloned()
                                    };

                                    if let Some(sender_arc) = sender_arc {
                                        let mut sender = sender_arc.lock().await;
                                        if let Err(err) =
                                            sender.send(Message::Text(response_text.into())).await
                                        {
                                            eprintln!(
                                                "Failed to send message to user {}: {}",
                                                sender_id, err
                                            );
                                        }
                                    }
                                }
                            }
                            _ => continue,
                        }
                    }
                }
            }
            _ => continue,
        }
    }
}
