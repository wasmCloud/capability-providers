use aws_sdk_dynamodb::model::AttributeValue;
use chrono::{Duration, Utc};
use futures::TryFutureExt;
use log::{debug, error};
use wasmbus_rpc::core::LinkDefinition;
use wasmbus_rpc::error::{RpcError, RpcResult};
use wasmcloud_interface_keyvalue::{GetResponse, SetRequest};

pub use config::AwsConfig;

mod config;

#[derive(Clone)]
pub struct DynamoDbClient {
    client: aws_sdk_dynamodb::Client,
    table_name: String,
    key_attribute: String,
    value_attribute: String,
    ttl_attribute: Option<String>,
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
            ttl_attribute: config.ttl_attribute.clone(),
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
        let sdk_response = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key(&self.key_attribute, AttributeValue::S(key.to_string()))
            .send()
            .map_err(|e| {


                error!("********* ERROR FROM AWS SDK *******");


                RpcError::Other(e.to_string())
            })
            .await?;

        match sdk_response.item {
            Some(i) => {
                let av = i.get(self.value_attribute.as_str()).ok_or_else(|| {
                    RpcError::Other(format!(
                        "record for key {} as no value attribute {}",
                        key.to_string(),
                        self.value_attribute
                    ))
                })?;

                let value = av.as_s()
                    .map_err(|_| RpcError::Other(format!(
                        "record for key {} has non-string value attribute {} - only string values are supported at this time",
                        key.to_string(), self.value_attribute)))?;

                Ok(GetResponse {
                    value: value.to_string(),
                    exists: true,
                })
            }
            None => Ok(GetResponse {
                value: "".to_string(),
                exists: false,
            }),
        }
    }

    pub async fn set(&self, arg: &SetRequest) -> RpcResult<()> {
        let mut put_item_request = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item(&self.key_attribute, AttributeValue::S(arg.key.to_string()))
            .item(
                &self.value_attribute,
                AttributeValue::S(arg.value.to_string()),
            );

        put_item_request = match (arg.expires, &self.ttl_attribute) {
            (0, _) => Ok(put_item_request),
            (_, None) => Err(RpcError::Other(
                "set request with expiry without configured ttl attribute".to_string(),
            )),
            (expiry, Some(ttl_attribute)) => {
                let expiry_timestamp =
                    Utc::now().timestamp() + Duration::seconds(expiry.into()).num_seconds();
                Ok(put_item_request.item(
                    ttl_attribute,
                    AttributeValue::N(expiry_timestamp.to_string()),
                ))
            }
        }?;

        put_item_request
            .send()
            .map_err(|e| RpcError::Other(e.to_string()))
            .await?;

        Ok(())
    }
}
