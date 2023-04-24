use std::fmt::Debug;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UserId {
    pub user_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterDeviceParams {
    pub device_name: String,
    pub mac: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterDeviceResult {
    pub device_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub mac: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserNonce {
    pub user_id: String,
    pub nonce: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserNonceResult {
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

// 服务端返回的数据类型
#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseParams<T: Debug + Serialize + Serialize> {
    pub id: u64,
    pub method: String,
    pub code: i32,
    pub result: T,
}
