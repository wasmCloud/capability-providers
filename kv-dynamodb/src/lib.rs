use std::{collections::HashMap, env};

use aws_sdk_dynamodb::model::AttributeValue;
use aws_types::{
    config::Config, credentials::SharedCredentialsProvider, region::Region,
};
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

    pub async fn get<TS: ToString + ?Sized + Sync>(&self, arg: &TS) -> RpcResult<GetResponse> {
        let &attribute_value = &self.value_attribute.as_str();
        match self.client.get_item()
            .table_name(&self.table_name)
            .key(&self.key_attribute, AttributeValue::S(arg.to_string()))
            .send()
            .await {
            Ok(response) => {

                // Option<HashMap<String,AttributeValue>> -> Option<Option<&AttributeValue>>
                if let Some(item) = response.item {
                    if let Some(attr) = item.get(attribute_value).cloned() {
                        if let Ok(v) = attr.as_s() {
                            Ok(GetResponse {
                                value: v.to_string(),
                                exists: true
                            })
                        } else {
                            Err(RpcError::Other("value not string".to_string()))
                        }
                    } else {
                        Err(RpcError::Other("item not found".to_string()))
                    }
                } else {
                    Err(RpcError::Other("item not found".to_string()))
                }
            }

            Err(e) => Err(RpcError::Other(e.to_string()))
        }
    }
}



