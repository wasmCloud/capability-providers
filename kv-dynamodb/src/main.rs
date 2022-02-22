//! DynamoDB implementation for wasmcloud:keyvalue.
//!
//! This implementation is multi-threaded and operations between different actors
//! use different connections and can run in parallel.
//! A single connection is shared by all instances of the same actor id (public key),
//! so there may be some brief lock contention if several instances of the same actor
//! are simultaneously attempting to communicate with Dynamo. See documentation
//! on the [exec](#exec) function for more information.
//!
//!
use std::{collections::HashMap, convert::Infallible, sync::Arc};

use aws_sdk_dynamodb::model::AttributeValue;
use tokio::sync::RwLock;
use wasmbus_rpc::provider::prelude::*;
use wasmcloud_interface_keyvalue::{
    GetResponse, IncrementRequest, KeyValue, KeyValueReceiver, ListAddRequest, ListDelRequest,
    ListRangeRequest, SetAddRequest, SetDelRequest, SetRequest, StringList,
};

use kv_dynamodb_lib::{AwsDynamoConfig, DynamoDbClient};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // handle lattice control messages and forward rpc to the provider dispatch
    // returns when provider receives a shutdown control message
    provider_main(KvDynamoProvider::default())?;

    eprintln!("KVDynamo provider exiting");
    Ok(())
}

/// Redis keyValue provider implementation.
#[derive(Default, Clone, Provider)]
#[services(KeyValue)]
struct KvDynamoProvider {
    // store redis connections per actor
    actors: Arc<RwLock<HashMap<String, DynamoDbClient>>>,
}

/// use default implementations of provider message handlers
impl ProviderDispatch for KvDynamoProvider {}

impl KvDynamoProvider {
    async fn client(&self, ctx: &Context) -> RpcResult<DynamoDbClient> {
        let actor_id = ctx
            .actor
            .as_ref()
            .ok_or_else(|| RpcError::InvalidParameter("no actor in request".to_string()))?;
        // get read lock on actor-client hashmap
        let rd = self.actors.read().await;
        let client = rd
            .get(actor_id)
            .ok_or_else(|| RpcError::InvalidParameter(format!("actor not linked:{}", actor_id)))?;
        Ok(client.clone())
    }
}

/// Handle provider control commands
/// put_link (new actor link command), del_link (remove link command), and shutdown
#[async_trait]
impl ProviderHandler for KvDynamoProvider {
    /// Provider should perform any operations needed for a new link,
    /// including setting up per-actor resources, and checking authorization.
    /// If the link is allowed, return true, otherwise return false to deny the link.
    async fn put_link(&self, ld: &LinkDefinition) -> RpcResult<bool> {
        let config = AwsDynamoConfig::from_values(&ld.values)?;
        let link = DynamoDbClient::new(config, Some(ld.clone())).await;

        let mut update_map = self.actors.write().await;
        update_map.insert(ld.actor_id.to_string(), link);

        Ok(true)
    }

    /// Handle notification that a link is dropped - close the connection
    async fn delete_link(&self, actor_id: &str) {
        let mut aw = self.actors.write().await;
        if let Some(conn) = aw.remove(actor_id) {
            log::info!("redis closing connection for actor {}", actor_id);
            drop(conn)
        }
    }

    /// Handle shutdown request by closing all connections
    async fn shutdown(&self) -> Result<(), Infallible> {
        let mut aw = self.actors.write().await;
        // empty the actor link data and stop all servers
        for (_, conn) in aw.drain() {
            drop(conn)
        }
        Ok(())
    }
}

// There are two api styles you can use for invoking redis. You can build any raw command
// as a string command and a sequence of args:
// ```
//     let mut cmd = redis::cmd("SREM");
//     let value: u32 = self.exec(ctx, &mut cmd.arg(&arg.set_name).arg(&arg.value)).await?;
// ```
// or you can call a method on Cmd, as in
// ```
//     let mut cmd = redis::Cmd::srem(&arg.set_name, &arg.value);
//     let value: u32 = self.exec(ctx, &mut cmd).await?;
//```
// The latter api style has better rust compile-time type checking for args.
// The rust docs for cmd and Cmd don't document arg types or return types.
// For that, you need to look at https://redis.io/commands#

/// Handle KeyValue methods that interact with redis
#[async_trait]
impl KeyValue for KvDynamoProvider {
    /// Increments a numeric value, returning the new value
    async fn increment(&self, ctx: &Context, arg: &IncrementRequest) -> RpcResult<i32> {
        unimplemented!()
    }

    /// Returns true if the store contains the key
    async fn contains<TS: ToString + ?Sized + Sync>(
        &self,
        ctx: &Context,
        arg: &TS,
    ) -> RpcResult<bool> {
        unimplemented!()
    }

    /// Deletes a key, returning true if the key was deleted
    async fn del<TS: ToString + ?Sized + Sync>(&self, ctx: &Context, arg: &TS) -> RpcResult<bool> {
        unimplemented!()
    }

    /// Gets a value for a specified key. If the key exists,
    /// the return structure contains exists: true and the value,
    /// otherwise the return structure contains exists == false.
    async fn get<TS: ToString + ?Sized + Sync>(
        &self,
        ctx: &Context,
        arg: &TS,
    ) -> RpcResult<GetResponse> {
        let client = self.client(ctx).await?;
        match client.client
            .get_item()
            .table_name(client.table_name)
            .key(client.key_attribute, AttributeValue::S(arg.to_string()))
            .send()
            .await {

            Ok(response) => {
                let v = response.clone().item
                    .map(|i| i.get(client.value_attribute.as_str())
                        .map(|a| a.as_s().unwrap().to_string())).unwrap();

                Ok(GetResponse {
                    value: v.clone().unwrap_or("".to_string()),
                    exists: v.clone().is_some()
                })
            }
            Err(e) => Err(RpcError::Other(e.to_string()))
        }


    }

    /// Append a value onto the end of a list. Returns the new list size
    async fn list_add(&self, ctx: &Context, arg: &ListAddRequest) -> RpcResult<u32> {
        unimplemented!()
    }

    /// Deletes a list and its contents
    /// input: list name
    /// returns: true if the list existed and was deleted
    async fn list_clear<TS: ToString + ?Sized + Sync>(
        &self,
        ctx: &Context,
        arg: &TS,
    ) -> RpcResult<bool> {
        unimplemented!()
    }

    /// Deletes an item from a list. Returns true if the item was removed.
    async fn list_del(&self, ctx: &Context, arg: &ListDelRequest) -> RpcResult<bool> {
        unimplemented!()
    }

    /// Retrieves a range of values from a list using 0-based indices.
    /// Start and end values are inclusive, for example, (0,10) returns
    /// 11 items if the list contains at least 11 items. If the stop value
    /// is beyond the end of the list, it is treated as the end of the list.
    async fn list_range(&self, ctx: &Context, arg: &ListRangeRequest) -> RpcResult<StringList> {
        unimplemented!()
    }

    /// Sets the value of a key.
    /// expires is an optional number of seconds before the value should be automatically deleted,
    /// or 0 for no expiration.
    async fn set(&self, ctx: &Context, arg: &SetRequest) -> RpcResult<()> {
        unimplemented!()
    }

    /// Add an item into a set. Returns number of items added
    async fn set_add(&self, ctx: &Context, arg: &SetAddRequest) -> RpcResult<u32> {
        unimplemented!()
    }

    /// Remove a item from the set. Returns
    async fn set_del(&self, ctx: &Context, arg: &SetDelRequest) -> RpcResult<u32> {
        unimplemented!()
    }

    /// Deletes a set and its contents
    /// input: set name
    /// returns: true if the set existed and was deleted
    async fn set_clear<TS: ToString + ?Sized + Sync>(
        &self,
        ctx: &Context,
        arg: &TS,
    ) -> RpcResult<bool> {
        unimplemented!()
    }

    async fn set_intersection(
        &self,
        ctx: &Context,
        arg: &StringList,
    ) -> Result<StringList, RpcError> {
        unimplemented!()
    }

    async fn set_query<TS: ToString + ?Sized + Sync>(
        &self,
        ctx: &Context,
        arg: &TS,
    ) -> RpcResult<StringList> {
        unimplemented!()
    }

    async fn set_union(&self, ctx: &Context, arg: &StringList) -> RpcResult<StringList> {
        unimplemented!()
    }
}
