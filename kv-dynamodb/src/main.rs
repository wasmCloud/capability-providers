//! DynamoDB implementation for wasmcloud:keyvalue.
//!
//! This implementation is multi-threaded and operations between different actors
//! use different clients and can run in parallel.
//! A single connection is shared by all instances of the same actor id (public key),
//! so there may be some brief lock contention if several instances of the same actor
//! are simultaneously attempting to communicate with Dynamo.
//!
//!
use std::{collections::HashMap, convert::Infallible, sync::Arc};

use tokio::sync::RwLock;
use wasmbus_rpc::provider::prelude::*;
use wasmcloud_interface_keyvalue::{
    GetResponse, IncrementRequest, KeyValue, KeyValueReceiver, ListAddRequest, ListDelRequest,
    ListRangeRequest, SetAddRequest, SetDelRequest, SetRequest, StringList,
};

use kv_dynamodb_lib::{AwsConfig, DynamoDbClient};

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
        let config = AwsConfig::from_values(&ld.values)?;
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

/// Handle KeyValue methods that interact with DynamoDB
#[async_trait]
impl KeyValue for KvDynamoProvider {
    /// Increments a numeric value, returning the new value
    async fn increment(&self, _ctx: &Context, _arg: &IncrementRequest) -> RpcResult<i32> {
        todo!()
    }

    /// Returns true if the store contains the key
    async fn contains<TS: ToString + ?Sized + Sync>(
        &self,
        _ctx: &Context,
        _arg: &TS,
    ) -> RpcResult<bool> {
        todo!()
    }

    /// Deletes a key, returning true if the key was deleted
    async fn del<TS: ToString + ?Sized + Sync>(
        &self,
        _ctx: &Context,
        _arg: &TS,
    ) -> RpcResult<bool> {
        todo!()
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
        client.get(arg).await
    }

    /// Append a value onto the end of a list. Returns the new list size
    async fn list_add(&self, _ctx: &Context, _arg: &ListAddRequest) -> RpcResult<u32> {
        todo!()
    }

    /// Deletes a list and its contents
    /// input: list name
    /// returns: true if the list existed and was deleted
    async fn list_clear<TS: ToString + ?Sized + Sync>(
        &self,
        _ctx: &Context,
        _arg: &TS,
    ) -> RpcResult<bool> {
        todo!()
    }

    /// Deletes an item from a list. Returns true if the item was removed.
    async fn list_del(&self, _ctx: &Context, _arg: &ListDelRequest) -> RpcResult<bool> {
        todo!()
    }

    /// Retrieves a range of values from a list using 0-based indices.
    /// Start and end values are inclusive, for example, (0,10) returns
    /// 11 items if the list contains at least 11 items. If the stop value
    /// is beyond the end of the list, it is treated as the end of the list.
    async fn list_range(&self, _ctx: &Context, _arg: &ListRangeRequest) -> RpcResult<StringList> {
        todo!()
    }

    /// Sets the value of a key.
    /// expires is an optional number of seconds before the value should be automatically deleted,
    /// or 0 for no expiration.
    async fn set(&self, ctx: &Context, arg: &SetRequest) -> RpcResult<()> {
        let client = self.client(ctx).await?;
        client.set(arg).await
    }

    /// Add an item into a set. Returns number of items added
    async fn set_add(&self, _ctx: &Context, _arg: &SetAddRequest) -> RpcResult<u32> {
        todo!()
    }

    /// Remove a item from the set. Returns
    async fn set_del(&self, _ctx: &Context, _arg: &SetDelRequest) -> RpcResult<u32> {
        todo!()
    }

    async fn set_intersection(
        &self,
        _ctx: &Context,
        _arg: &StringList,
    ) -> Result<StringList, RpcError> {
        todo!()
    }

    async fn set_query<TS: ToString + ?Sized + Sync>(
        &self,
        _ctx: &Context,
        _arg: &TS,
    ) -> RpcResult<StringList> {
        todo!()
    }

    async fn set_union(&self, _ctx: &Context, _arg: &StringList) -> RpcResult<StringList> {
        todo!()
    }

    /// Deletes a set and its contents
    /// input: set name
    /// returns: true if the set existed and was deleted
    async fn set_clear<TS: ToString + ?Sized + Sync>(
        &self,
        _ctx: &Context,
        _arg: &TS,
    ) -> RpcResult<bool> {
        todo!()
    }
}
