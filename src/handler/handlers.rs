use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{State, TypedHeader};
use axum::response::IntoResponse;
use futures::stream::{SplitSink, SplitStream};
use tracing::Level;
//allows to extract the IP of connecting user
use axum::extract::connect_info::ConnectInfo;

use std::net::SocketAddr;
use std::ops::ControlFlow;

//allows to split the websocket stream into separate TX and RX branches
use futures::{sink::SinkExt, stream::StreamExt};

use crate::jwt::new_token;
use crate::types::{
    DeviceInfo, GetNonceParams, LoginParams, LoginResult, RegisterDeviceParams,
    RegisterDeviceResult, RequestParams, ResponseParams, UserNonceResult,
};
use crate::utils::{self, verify_signature};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(db): State<crate::db::DB>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");

    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr, db))
}

async fn handle_socket(mut socket: WebSocket, who: SocketAddr, client: crate::db::DB) {
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
        loop {
            tokio::select! {
                    result = receiver.next() => {
                        match result {
                            Some(Ok(msg)) =>  {
                                cnt += 1;
                                // print message and break if instructed to do so
                                if process_message(msg, who, &mut sender, &mut receiver, &client).await.is_break() {
                                    return cnt;
                                }
                            },
                            Some(Err(e)) => {
                                println!("### receive next Err: {:?}",e)
                            },
                            None => {
                                println!("### receive None");
                                break cnt
                            }
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
    _receiver: &mut SplitStream<WebSocket>,
    db: &crate::db::DB,
) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            let t = t.trim();
            println!(">>> {} sent str: {:?}", who, t);

            if t == "ping" {
                if let Err(e) = sender.send(Message::Text("pong".into())).await {
                    tracing::event!(Level::ERROR, "Send message failed {:?}", e);
                }
                return ControlFlow::Continue(());
            }

            let v: RequestParams = match serde_json::from_str(&t) {
                Ok(v) => v,
                Err(e) => {
                    tracing::event!(Level::ERROR, "Unmarshal json failed: {:?}", e);
                    return ControlFlow::Continue(());
                }
            };

            if &v.method == "getNonce" {
                let params: GetNonceParams = match serde_json::from_value(v.params.clone()) {
                    Ok(params) => params,
                    Err(e) => {
                        tracing::event!(Level::ERROR, "Unmarshal failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                };

                let nonce = match db.get_nonce(&params.user_id).await {
                    Ok(nonce) => nonce,
                    Err(e) => {
                        tracing::event!(Level::ERROR, "Unmarshal failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                };
                let result = serde_json::to_string(&ResponseParams {
                    id: v.id,
                    method: v.method.clone(),
                    code: 0,
                    result: &UserNonceResult { nonce: nonce.to_string() },
                })
                .unwrap();

                if let Err(e) = sender.send(Message::Text(result)).await {
                    tracing::event!(Level::ERROR, "Send message failed {:?}", e);
                };
                return ControlFlow::Continue(());
            }
            if &v.method == "registerDevice" {
                let params: RegisterDeviceParams = match serde_json::from_value(v.params.clone()) {
                    Ok(params) => params,
                    Err(e) => {
                        println!("Unmarshal failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                };

                // 获取nonce
                let device_id = match db.new_device_id().await {
                    Ok(device_id) => device_id,
                    Err(e) => {
                        tracing::event!(Level::ERROR, "Get new deviceId failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                };
                let result = serde_json::to_string(&ResponseParams {
                    id: v.id,
                    method: v.method.clone(),
                    code: 0,
                    result: &RegisterDeviceResult { device_id: device_id.clone() },
                })
                .unwrap();

                if let Err(e) = db
                    .update_device(DeviceInfo {
                        device_id,
                        device_name: params.device_name,
                        mac: params.mac,
                        online: true,
                        add_time: utils::now(),
                        ..Default::default()
                    })
                    .await
                {
                    tracing::event!(Level::ERROR, "Unmarshal failed: {:?}", e);
                    return ControlFlow::Continue(());
                };

                sender.send(Message::Text(result)).await.unwrap();
                return ControlFlow::Continue(());
            }
            if &v.method == "login" {
                let params: LoginParams = match serde_json::from_value(v.params.clone()) {
                    Ok(params) => params,
                    Err(e) => {
                        tracing::event!(Level::ERROR, "Unmarshal failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                };
                // 获取并检查nonce
                let nonce = match db.get_nonce(&params.user_id).await {
                    Ok(nonce) => nonce,
                    Err(e) => {
                        tracing::event!(Level::ERROR, "GetNonce failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                };
                if params.nonce <= nonce {
                    tracing::event!(Level::ERROR, "Invalid nonce");
                    return ControlFlow::Continue(());
                }
                // 检查签名
                match verify_signature(
                    &params.user_id,
                    &params.nonce.to_string(),
                    &params.signature,
                ) {
                    Err(e) => {
                        tracing::event!(Level::ERROR, "Verify signature failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                    Ok(false) => {
                        tracing::event!(Level::ERROR, "Verify signature failed: invalid signature");
                        return ControlFlow::Continue(());
                    }
                    Ok(true) => {}
                }

                let token = new_token(params.user_id.clone(), params.device_id.clone());

                // 更新nonce
                if let Err(e) = db.update_nonce(&params.user_id, params.nonce).await {
                    tracing::event!(Level::ERROR, "Update nonce failed: {:?}", e);
                    return ControlFlow::Continue(());
                };

                let result = serde_json::to_string(&ResponseParams {
                    id: v.id,
                    method: v.method.as_str().to_owned(),
                    code: 0,
                    result: &LoginResult { token },
                })
                .unwrap();

                if let Err(e) = sender.send(Message::Text(result)).await {
                    tracing::event!(Level::ERROR, "Send message failed {:?}", e);
                };
                return ControlFlow::Continue(());
            }

            // {"id":1,"method":"imOnline",
            // "token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJVc2VySWQiOiI1RWJtMTNjVWVTRUZ5QWZDM29Td1phVnVYS29kYmQ3OVc4RkhiWGFQaUc0NThoZkoiLCJEZXZpY2VJZCI6IjQ1NTEwNjg5OCIsImlzcyI6ImRlZXBsaW5rIiwic3ViIjoiVXNlciB0b2tlbiIsImV4cCI6MTcxNzIwMDk0NywiaWF0IjoxNjgxMjAwOTQ3fQ.EdldPibkgZuoBplCPp00YeHUZQwuL1sje90Ee5Obods",
            // "params":{"device_id":"684060212"}}
            if &v.method == "imOnline" {
                // TODO: 1. verify token
                // 2. 更新设备Online状态
                // return {"id":1,"method":"imOnline","code":0,"result":{"message":"Ok"}}
            }

            // 绑定设备
            // {"id":1,"method":"bindDevice",
            // "token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJVc2VySWQiOiI1RWJtMTNjVWVTRUZ5QWZDM29Td1phVnVYS29kYmQ3OVc4RkhiWGFQaUc0NThoZkoiLCJEZXZpY2VJZCI6IjQ1NTEwNjg5OCIsImlzcyI6ImRlZXBsaW5rIiwic3ViIjoiVXNlciB0b2tlbiIsImV4cCI6MTcxNzIwMDk0NywiaWF0IjoxNjgxMjAwOTQ3fQ.EdldPibkgZuoBplCPp00YeHUZQwuL1sje90Ee5Obods",
            // "params":{"device_id":"684060212","device_name":"boob-manjaro"}}
            if &v.method == "bindDevice" {}

            // # 解绑设备
            // {"id":1,"method":"unbindDevice",
            // "token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJVc2VySWQiOiI1RWJtMTNjVWVTRUZ5QWZDM29Td1phVnVYS29kYmQ3OVc4RkhiWGFQaUc0NThoZkoiLCJEZXZpY2VJZCI6IjQ1NTEwNjg5OCIsImlzcyI6ImRlZXBsaW5rIiwic3ViIjoiVXNlciB0b2tlbiIsImV4cCI6MTcxNzIwMDk0NywiaWF0IjoxNjgxMjAwOTQ3fQ.EdldPibkgZuoBplCPp00YeHUZQwuL1sje90Ee5Obods",
            // "params":{"device_id":"684060212"}}
            if &v.method == "unbindDevice" {}

            // # 获取设备列表
            // {"id":1,"method":"getDeviceList",
            // "token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJVc2VySWQiOiI1RWJtMTNjVWVTRUZ5QWZDM29Td1phVnVYS29kYmQ3OVc4RkhiWGFQaUc0NThoZkoiLCJEZXZpY2VJZCI6IjQ1NTEwNjg5OCIsImlzcyI6ImRlZXBsaW5rIiwic3ViIjoiVXNlciB0b2tlbiIsImV4cCI6MTcxNzIwMDk0NywiaWF0IjoxNjgxMjAwOTQ3fQ.EdldPibkgZuoBplCPp00YeHUZQwuL1sje90Ee5Obods",
            // "params":{}}
            // {"id":1,"method":"getDeviceList","code":0,"result":{"device_list":[{"device_id":"684060212","device_name":"boob-manjaro","mac":"00:2B:67:6F:74:72","online":false,"add_time":"2023-04-13T10:04:49.19Z","update_time":"2023-04-13T10:05:32.34Z"}]}}
            if &v.method == "getDeviceList" {}
        }
        _ => {
            tracing::event!(Level::WARN, "Not allowed message: {:?}", msg);
        }
    }
    ControlFlow::Continue(())
}
