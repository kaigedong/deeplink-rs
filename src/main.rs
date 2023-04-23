use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::TypedHeader;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures::stream::{SplitSink, SplitStream};
use serde::{Deserialize, Serialize};
use serde_json::{json, Result, Value};

use std::net::SocketAddr;
use std::ops::ControlFlow;

//allows to extract the IP of connecting user
use axum::extract::connect_info::ConnectInfo;

//allows to split the websocket stream into separate TX and RX branches
use futures::{sink::SinkExt, stream::StreamExt};

mod mongo;

#[derive(Serialize, Deserialize)]
struct Person {}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/ws", get(ws_handler));
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");

    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

async fn handle_socket(mut socket: WebSocket, who: SocketAddr) {
    //send a ping (unsupported by some browsers) just to kick things off and get a response
    if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
        println!("Pinged {}...", who);
    } else {
        println!("Could not send ping {}!", who);
        // no Error here since the only thing we can do is to close the connection.
        // If we can not send messages, there is no way to salvage the statemachine anyway.
        return;
    }

    // By splitting socket we can send and receive at the same time.
    let (mut sender, mut receiver) = socket.split();

    // This second task will receive messages from client and print them on server console
    let mut recv_task = tokio::spawn(async move {
        let mut cnt = 0;
        // TODO: 需要将这里写一个select，根据事件先后来处理
        loop {
            tokio::select! {
                    Some(Ok(msg)) = receiver.next() => {
                                cnt += 1;
                                // print message and break if instructed to do so
                                if process_message(msg, who, &mut sender, &mut receiver).await.is_break() {
                                    return cnt;
                                }
                            },
                // 添加TODO: 加一个管道，当管道中有数据时，读取数据并处理
            }
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        rv_b = (&mut recv_task) => {
            match rv_b {
                Ok(b) => println!("Received {} messages", b),
                Err(b) => println!("Error receiving messages {:?}", b)
            }
        }
    }
}

/// helper to print contents of messages to stdout. Has special treatment for Close.
async fn process_message(
    msg: Message,
    who: SocketAddr,
    sender: &mut SplitSink<WebSocket, Message>,
    receiver: &mut SplitStream<WebSocket>,
) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            if t.trim() == "ping" {
                if let Err(e) = sender.send(Message::Text("pong".into())).await {
                    println!("Send message failed {:?}", e);
                }
                return ControlFlow::Continue(());
            }

            // Parse the string of data into serde_json::Value.
            let v: Value = serde_json::from_str(&t).unwrap();
            // Access parts of the data by indexing with square brackets.
            println!("Please call {} at the number {}", v["method"], v["phones"][0]);
            if v["method"] == "registerDevice" {
                let params: Person = serde_json::from_value(v).unwrap();
            }

            println!(">>> {} sent str: {:?}", who, t);
        },
        _ => {
            println!("Not allowed message");
        },
    }
    ControlFlow::Continue(())
}
