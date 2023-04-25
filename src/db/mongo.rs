use anyhow::{anyhow, Error, Result};
use mongodb::{
    bson::doc,
    options::{ClientOptions, ServerApi, ServerApiVersion, UpdateOptions},
    results::UpdateResult,
    Client, Collection, Database,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use tracing::Level;

use crate::{
    types::{DeviceInfo, UserNonce},
    utils,
};

#[derive(Clone, Debug)]
pub struct DB {
    pub db: Database,
}

pub async fn init_mongo() -> mongodb::error::Result<Database> {
    let uri = "mongodb://bobo:boboPassword@localhost:27017/";

    let mut client_options = ClientOptions::parse(uri).await?;
    let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
    client_options.server_api = Some(server_api);

    let client = Client::with_options(client_options)?;
    client.database("admin").run_command(doc! {"ping": 1}, None).await?;
    tracing::event!(Level::DEBUG, "Pinged your deployment. Connected to MongoDB!");
    Ok(client.database("test"))
}

impl DB {
    pub async fn new() -> Self {
        match init_mongo().await {
            Ok(db) => DB { db },
            Err(e) => {
                tracing::event!(Level::ERROR, "{:?}", e);
                std::process::exit(-1);
            }
        }
    }

    pub async fn update_nonce(&self, user_id: &str, nonce: u64) -> Result<UpdateResult, Error> {
        let collection: Collection<UserNonce> = self.db.collection("nonce");

        let mut options = UpdateOptions::default();
        options.upsert = Some(true);
        collection
            .update_one(
                doc! {"user_id":user_id},
                doc! {"$set": { "user_id": user_id, "nonce": nonce.to_string() }},
                options,
            )
            .await
            .map_err(|e| anyhow!(e))
    }

    pub async fn update_device(&self, mut device_info: DeviceInfo) -> Result<UpdateResult, Error> {
        let typed_collection = self.db.collection::<DeviceInfo>("device");

        device_info.update_time = utils::now();

        let mut options = UpdateOptions::default();
        options.upsert = Some(true);
        typed_collection
            .update_one(
                doc! {"device_id": device_info.device_id.clone()},
                doc! {"$set": bson::to_bson(&device_info)?},
                options,
            )
            .await
            .map_err(|e| anyhow!(e))
    }

    pub async fn get_nonce(&self, user_id: &str) -> Result<u64, Error> {
        let typed_collection = self.db.collection::<UserNonce>("nonce");
        let filter = doc! { "user_id": user_id };
        let result = typed_collection.find_one(filter, None).await.map_err(|e| anyhow!(e))?;
        match result {
            Some(nonce) => Ok(nonce.nonce.parse::<u64>()?),
            None => Ok(0),
        }
    }

    pub async fn new_device_id(&self) -> Result<String, Error> {
        for _ in 0..50 {
            let mut rng = StdRng::from_entropy();
            let num = rng.gen_range(100_000_000..1_000_000_000);
            let exist = self.device_id_exist(&num.to_string()).await.unwrap();
            if !exist {
                return Ok(num.to_string());
            }
        }
        return Err(anyhow!("Err"));
    }

    pub async fn device_id_exist(&self, device_id: &str) -> Result<bool, Error> {
        let typed_collection = self.db.collection::<DeviceInfo>("device");
        let filter = doc! {"device_id": device_id};
        let result = typed_collection.find_one(filter, None).await?;
        return match result {
            Some(_) => Ok(true),
            None => Ok(false),
        };
    }
}
