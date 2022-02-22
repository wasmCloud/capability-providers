use std::{collections::HashMap, env};

use aws_types::{
    config::Config as AwsConfig, credentials::SharedCredentialsProvider, region::Region,
};
use log::debug;
use serde::Deserialize;
use wasmbus_rpc::core::LinkDefinition;
use wasmbus_rpc::error::{RpcError, RpcResult};

pub use config::AwsDynamoConfig;

mod config;

#[derive(Clone)]
pub struct DynamoDbClient {
    pub client: aws_sdk_dynamodb::Client,
    pub table_name: String,
    pub key_attribute: String,
    pub value_attribute: String,
    pub link: Option<LinkDefinition>
}

impl DynamoDbClient {
    pub async fn new(config: config::AwsDynamoConfig, ld: Option<LinkDefinition>) -> Self {
        let dynamo_config = aws_sdk_dynamodb::Config::from(&config.clone().configure_aws().await);
        let dynamo_client = aws_sdk_dynamodb::Client::from_conf(dynamo_config);
        DynamoDbClient {
            client: dynamo_client,
            table_name: config.table_name.clone(),
            key_attribute: config.key_attribute.clone(),
            value_attribute: config.value_attribute.clone(),
            link: ld,
        }
    }

    /// async implementation of Default
    pub async fn async_default() -> Self {
        Self::new(config::AwsDynamoConfig::default(), None).await
    }

    /// Perform any cleanup necessary for a link + s3 connection
    pub async fn close(&self) {
        if let Some(ld) = &self.link {
            debug!("kv-dynamodb dropping linkdef for {}", ld.actor_id);
        }
        // If there were any https clients, caches, or other link-specific data,
        // we would delete those here
    }
}



