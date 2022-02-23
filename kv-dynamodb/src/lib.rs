use std::{collections::HashMap, env};

use aws_sdk_dynamodb::model::AttributeValue;
use aws_types::{config::Config, credentials::SharedCredentialsProvider, region::Region};
use log::debug;
use serde::Deserialize;
use wasmbus_rpc::core::LinkDefinition;
use wasmbus_rpc::error::{RpcError, RpcResult};
use wasmbus_rpc::RpcError::Rpc;
use wasmcloud_interface_keyvalue::GetResponse;

pub use config::AwsConfig;

mod config;

#[derive(Clone)]
pub struct DynamoDbClient {
    client: aws_sdk_dynamodb::Client,
    table_name: String,
    key_attribute: String,
    value_attribute: String,
    pub link: Option<LinkDefinition>,
}

impl DynamoDbClient {
    pub async fn new(config: config::AwsConfig, ld: Option<LinkDefinition>) -> Self {
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
        Self::new(config::AwsConfig::default(), None).await
    }

    /// Perform any cleanup necessary for a link + s3 connection
    pub async fn close(&self) {
        if let Some(ld) = &self.link {
            debug!("kv-dynamodb dropping linkdef for {}", ld.actor_id);
        }
        // If there were any https clients, caches, or other link-specific data,
        // we would delete those here
    }

    pub async fn get<TS: ToString + ?Sized + Sync>(&self, key: &TS) -> RpcResult<GetResponse> {
        let &attribute_value = &self.value_attribute.as_str();
        match self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key(&self.key_attribute, AttributeValue::S(key.to_string()))
            .send()
            .await
        {
            Ok(response) => {
                let item = response.item.ok_or(RpcError::Other(format!(
                    "no record found for key: {}",
                    key.to_string()
                )))?;

                let av = item.get(attribute_value).ok_or(RpcError::Other(format!(
                    "record for key {} as no value attribute {}",
                    key.to_string(),
                    attribute_value
                )))?;

                let value = av.as_s()
                    .map_err(|_| RpcError::Other((format!(
                        "record for key {} has non-string value attribute {} - only string values are supported at this time",
                        key.to_string(), attribute_value))))?;

                Ok(GetResponse {
                    value: value.to_string(),
                    exists: true,
                })
            }

            Err(e) => Err(RpcError::Other(e.to_string())),
        }
    }
}
