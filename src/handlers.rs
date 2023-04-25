use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{State, TypedHeader};
use axum::response::IntoResponse;
use futures::stream::{SplitSink, SplitStream};
use serde_json::Value;

use std::net::SocketAddr;
use std::ops::ControlFlow;

//allows to extract the IP of connecting user
use axum::extract::connect_info::ConnectInfo;

//allows to split the websocket stream into separate TX and RX branches
use futures::{sink::SinkExt, stream::StreamExt};

use crate::jwt::new_token;
use crate::types::{
    DeviceInfo, LoginParams, LoginResult, RegisterDeviceParams, RegisterDeviceResult,
    ResponseParams, UserId, UserNonceResult,
};
use crate::utils::{self, verify_signature};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(db): State<crate::mongo::DB>,
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

async fn handle_socket(mut socket: WebSocket, who: SocketAddr, client: crate::mongo::DB) {
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

    println!("###### Hook1");

    // This second task will receive messages from client and print them on server console
    let mut recv_task = tokio::spawn(async move {
        println!("###### Hook2");
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
    db: &crate::mongo::DB,
) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            let t = t.trim();
            if t == "ping" {
                if let Err(e) = sender.send(Message::Text("pong".into())).await {
                    println!("Send message failed {:?}", e);
                }
                return ControlFlow::Continue(());
            }

            let v: Value = match serde_json::from_str(&t) {
                Ok(v) => v,
                Err(e) => {
                    println!("### Unmarshal json failed: {:?}", e);
                    return ControlFlow::Continue(());
                }
            };

            // {"id": 1,"method": "getNonce","token": "","params": {"user_id": "5Ebm13cUeSEFyAfC3oSwZaVuXKodbd79W8FHbXaPiG458hfJ"}}
            if v["method"] == "getNonce" {
                let params: UserId = match serde_json::from_value(v["params"].clone()) {
                    Ok(params) => params,
                    Err(e) => {
                        println!("Unmarshal failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                };

                // 获取nonce
                let nonce = db.get_nonce(&params.user_id).await.unwrap();
                let result = serde_json::to_string(&ResponseParams {
                    id: v["id"].as_u64().unwrap(),
                    method: v["method"].as_str().unwrap().to_owned(),
                    code: 0,
                    result: &UserNonceResult { nonce: nonce.to_string() },
                })
                .unwrap();

                sender.send(Message::Text(result)).await.unwrap();
            }
            // {"id":1,"method":"registerDevice","token":"","params":{"device_name":"bobo-manjaro","mac":"00:2B:67:6F:74:72"}}
            if v["method"] == "registerDevice" {
                let params: RegisterDeviceParams = match serde_json::from_value(v["params"].clone())
                {
                    Ok(params) => params,
                    Err(e) => {
                        println!("Unmarshal failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                };

                // 获取nonce
                let device_id = db.new_device_id().await.unwrap();
                let result = serde_json::to_string(&ResponseParams {
                    id: v["id"].as_u64().unwrap(),
                    method: v["method"].as_str().unwrap().to_owned(),
                    code: 0,
                    result: &RegisterDeviceResult { device_id: device_id.clone() },
                })
                .unwrap();

                if let Err(_) = db
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
                    println!("######### update device failed")
                };

                sender.send(Message::Text(result)).await.unwrap();
            }
            // {"id":1,"method":"login","token":"",
            // "params":{"user_id":"5Ebm13cUeSEFyAfC3oSwZaVuXKodbd79W8FHbXaPiG458hfJ","device_id":"684060212","nonce":1,
            // "signature":"0xc46eee1875fd3a2ac7f4877080e17ecea2ab66f51bdaa1581acf92ca65323f5f415314242d5513c070ef7fbd78593c0a9116fdeb6288ff28d67a503f7e23bf84"}}
            if v["method"] == "login" {
                let params: LoginParams = match serde_json::from_value(v["params"].clone()) {
                    Ok(params) => params,
                    Err(e) => {
                        println!("Unmarshal failed: {:?}", e);
                        return ControlFlow::Continue(());
                    }
                };
                // 获取并检查nonce
                let nonce = db.get_nonce(&params.user_id).await.unwrap();
                if params.nonce <= nonce {
                    println!();
                    return ControlFlow::Continue(());
                }
                // 检查签名
                match verify_signature(
                    &params.user_id,
                    &params.nonce.to_string(),
                    &params.signature,
                ) {
                    Err(e) => {
                        return ControlFlow::Continue(());
                    }
                    Ok(true) => {}
                    Ok(false) => {
                        return ControlFlow::Continue(());
                    }
                }

                // TODO: 生成Token
                let token = new_token(params.user_id.clone(), params.device_id.clone());

                // 更新nonce
                if let Err(e) = db.update_nonce(&params.user_id, params.nonce).await {
                    println!("##### update failed {:?}", e);
                };

                let result = serde_json::to_string(&ResponseParams {
                    id: v["id"].as_u64().unwrap(),
                    method: v["method"].as_str().unwrap().to_owned(),
                    code: 0,
                    result: &LoginResult { token },
                })
                .unwrap();
                sender.send(Message::Text(result)).await.unwrap();
            }

            // // 更新nonce
            // if let Err(e) = db.update_nonce(&params.user_id, nonce + 1).await {
            //     println!("##### update failed {:?}", e);
            // };
            // let nonce = db.get_nonce(&params.user_id).await.unwrap();
            // sender
            //     .send(Message::Text((nonce + 1).to_string()))
            //     .await
            //     .unwrap();

            println!(">>> {} sent str: {:?}", who, t);
        }
        _ => {
            println!("Not allowed message {:?}", msg);
        }
    }
    ControlFlow::Continue(())
}
