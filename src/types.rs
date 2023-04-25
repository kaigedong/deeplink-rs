use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;

#[derive(Debug, Serialize, Deserialize)]
pub struct GetNonceParams {
    pub user_id: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub mac: String,
    pub online: bool,
    pub add_time: String,
    pub update_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserNonce {
    pub user_id: String,
    pub nonce: String,
}

pub enum RequestMethod {
    GetNonce,
}

impl TryFrom<&str> for RequestMethod {
    type Error = anyhow::Error;
    fn try_from(method: &str) -> Result<Self, Self::Error> {
        let err = || anyhow!(format!("Method not found: {}", method));

        if method == "getNonce" {
            return Ok(Self::GetNonce);
        } else {
            return Err(err());
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterDeviceParams {
    pub device_name: String,
    pub mac: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginParams {
    pub user_id: String,
    pub device_id: String,
    pub nonce: u64,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterDeviceResult {
    pub device_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserNonceResult {
    pub nonce: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResult {
    pub token: String,
}

// 服务端返回的数据类型
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ResponseParams<T: Debug + Serialize + Serialize> {
    pub id: u64,
    pub method: String,
    pub code: i32,
    pub result: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestParams {
    pub id: u64,
    pub method: String,
    pub token: String,
    pub params: Value,
}
