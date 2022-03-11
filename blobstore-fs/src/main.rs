//! blobstore-fs capability provider
//!
//!

#[allow(unused_imports)]
use log::{info, error};
use std::time::SystemTime;
#[allow(unused_imports)]
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{Write, BufReader,},
    path::{Path, PathBuf},
    sync::Arc,
    fs::{
        read,
        read_dir,
        remove_file, 
        File,
        metadata,
    },
};
use std::io::Read;
use tokio::sync::RwLock;
use wasmbus_rpc::provider::prelude::*;
use wasmbus_rpc::Timestamp;
use wasmcloud_interface_blobstore::*;
mod fs_utils;
pub use fs_utils::all_dirs;

#[allow(unused)]
const CAPABILITY_ID: &str = "wasmcloud:blobstore";
#[allow(unused)]
const FIRST_SEQ_NBR: u64 = 0;

// main (via provider_main) initializes the threaded tokio executor,
// listens to lattice rpcs, handles actor links,
// and returns only when it receives a shutdown message
//
fn main() -> Result<(), Box<dyn std::error::Error>> {
    provider_main(FsProvider::default())?;

    eprintln!("fs provider exiting");
    Ok(())
}

pub type ChunkOffsetKey = (String, u64);

#[derive(Default, Clone)]
#[allow(dead_code)]
struct FsProviderConfig {
    ld: LinkDefinition,
    root: PathBuf,
}


/// fs capability provider implementation
#[allow(dead_code)]
#[derive(Clone, Provider)]
#[services(Blobstore)]
struct FsProvider {
    config:          Arc<RwLock<HashMap<String, FsProviderConfig>>>,
    upload_chunks:   Arc<RwLock<HashMap<ChunkOffsetKey, Chunk>>>,
    download_chunks: Arc<RwLock<HashMap<ChunkOffsetKey, Chunk>>>,
}

impl Default for FsProvider {
    fn default() -> Self {
        FsProvider {
            config:          Arc::new(RwLock::new(HashMap::new())),
            upload_chunks:   Arc::new(RwLock::new(HashMap::new())),
            download_chunks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}



impl FsProvider {

    /// Get actor id string based on context value
    async fn get_actor_id(&self, ctx: &Context) -> RpcResult<String> {

        let actor_id = match &ctx.actor {
            Some(id) => id.clone(),
            None => {
                return Err(RpcError::InvalidParameter(String::from("No actor id found")));
            },
        };
        Ok(actor_id)
    }

    async fn get_ld(&self, ctx: &Context) -> RpcResult<LinkDefinition> {
        let actor_id = self.get_actor_id(ctx).await?;
        let conf_map = self.config.read().await;
        let conf = conf_map.get(&actor_id);
        let ld = match conf {
            Some(config) => config.ld.clone(),
            None => {
                return Err(RpcError::InvalidParameter(String::from("No root configuration found")));
            },
        };
        Ok(ld)
    }

    async fn get_root(&self, ctx: &Context) -> RpcResult<PathBuf> {
        let actor_id = self.get_actor_id(ctx).await?;
        let conf_map =  self.config.read().await;
        let root  = match conf_map.get(&actor_id) {
            Some(config) => config.root.clone(),
            None => {
                return Err(RpcError::InvalidParameter(String::from("No root configuration found")));
            },
        };
        Ok(root)
    }

    /// Stores a file chunk in right order.
    async fn store_chunk(&self, ctx: &Context, chunk: &Chunk, _stream_id: &Option<String>) -> RpcResult<()> {

        let root = self.get_root(ctx).await?;
        let cdir = Path::new(&root).join(&chunk.container_id);
        let bfile = Path::join(&cdir, &chunk.object_id);

        // create an empty file if it's the first
        if chunk.offset == 0 {
            let resp = std::fs::write(&bfile, &[]);
            if resp.is_err() {
                let error_string = format!("Could not create file: {:?}", bfile).to_string();
                error!("{:?}", &error_string);
                return Err(RpcError::InvalidParameter(error_string));
            }
        }

        let _upload_chunks = self.upload_chunks.write().await;


    
        let bpath = Path::join(
            &Path::join(
                &root,
                &chunk.container_id,
                ),
        &chunk.object_id
        );

        let mut file = OpenOptions::new().create(false).append(true).open(bpath)?;
        info!("Receiving file chunk offset {} for {}/{}, size {}", chunk.offset, 
                                                                chunk.container_id, 
                                                                chunk.object_id, 
                                                                chunk.bytes.len());
    
    
        let count = file.write(chunk.bytes.as_ref())?;
        if count != chunk.bytes.len() {
            let msg = format!("Failed to fully write chunk: {} of {} bytes",
                                    count,
                                    chunk.bytes.len()
                                    );
            error!("{}", &msg);
            return Err(msg.into());
        }

        Ok(())
    
    }

    /// Sends bytes to actor in a single rpc message.
    /// If successful, returns number of bytes sent (same as chunk.content_length)
    #[allow(unused)]
    async fn send_chunk(&self, ctx: &Context, chunk: Chunk) -> Result<u64, RpcError> {
        let ld = self.get_ld(ctx).await?;
        let receiver = ChunkReceiverSender::for_actor(&ld);
        if let Err(e) = receiver.receive_chunk(ctx, &chunk).await {
            let err = format!(
                "sending chunk error: Container({}) Object({}) to Actor({}): {:?}",
                &chunk.container_id, &chunk.object_id, &ld.actor_id, e
            );
            error!("{}", &err);
            Err(RpcError::Rpc(err))
        } else {
            Ok(chunk.bytes.len() as u64)
        }
    }


    fn get_root_from_ld(ld: &LinkDefinition) -> PathBuf {
        let mut root = PathBuf::from(match ld.values.get("ROOT") {
            Some(v) => Path::new(v),
            None => Path::new("/tmp"),     // perhaps not portable across different systems
        });

        root.push(ld.actor_id.clone());
        root
    }
}


/// use default implementations of provider message handlers
impl ProviderDispatch for FsProvider {}

#[async_trait]
impl ProviderHandler for FsProvider {
    /// The fs provider has one configuration parameter, the root of the file system
    /// 
    async fn put_link(&self, ld: &LinkDefinition) -> RpcResult<bool> {

        let root = Self::get_root_from_ld(ld);

        info!("File System Blob Store Container Root: '{:?}'", &root);

        self.config
            .write()
            .await
            .insert(ld.actor_id.clone(), FsProviderConfig {
                                                    ld: ld.clone(),
                                                    root,
                                                }
            );

        Ok(true)
    }
}

/// Handle Factorial methods
#[async_trait]
impl Blobstore for FsProvider {

    /// Returns whether the container exists
    #[allow(unused)]
    async fn container_exists(&self, ctx: &Context, arg: &ContainerId) -> RpcResult<bool> {
        info!("Called container_exists({:?})", arg);

        let root = self.get_root(ctx).await?;
        let cdir = Path::new(&root).join(&arg);

        match read_dir(&cdir) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Creates a container by name, returning success if it worked
    /// Note that container names may not be globally unique - just unique within the
    /// "namespace" of the connecting actor and linkdef
    async fn create_container(&self, ctx: &Context, arg: &ContainerId) -> RpcResult<()> {
        let root = self.get_root(ctx).await?;
        let cdir = Path::new(&root).join(arg.to_string());

        info!("create dir: {:?}", cdir);

        match std::fs::create_dir_all(cdir) {
            Ok(()) => Ok(()),
            Err(e) => Err(RpcError::InvalidParameter(format!("Could not create container: {:?}", e)))
        }
    }

    /// Retrieves information about the container.
    /// Returns error if the container id is invalid or not found.
    #[allow(unused)]
    async fn get_container_info(
        &self,
        ctx: &Context,
        arg: &ContainerId,
    ) -> RpcResult<ContainerMetadata> {

        let root = self.get_root(ctx).await?;
        let dir_path = Path::new(&root).join(&arg);

        let dir_info = metadata(dir_path)?;

        let modified = match  dir_info.modified()?.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(s) => Timestamp { sec: s.as_secs() as i64, nsec: 0u32 },
            Err(e) => return Err(RpcError::InvalidParameter(format!("{:?}", e))),
        };

        Ok(ContainerMetadata {
            container_id: arg.clone(),
            created_at: Some(modified),
        })
    }

    /// Returns list of container ids
    #[allow(unused)]
    async fn list_containers(&self, ctx: &Context) -> RpcResult<ContainersInfo> {

        let root = self.get_root(ctx).await?;

        let containers = all_dirs(&Path::new(&root), &root).iter()
            .map(|c| {
                ContainerMetadata {
                    container_id: c.as_path().display().to_string(),
                    created_at: None,
                }
            })
            .collect();

        Ok(containers)

    }

    /// Empty and remove the container(s)
    /// The MultiResult list contains one entry for each container
    /// that was not successfully removed, with the 'key' value representing the container name.
    /// If the MultiResult list is empty, all container removals succeeded.
    #[allow(unused)]
    async fn remove_containers(&self, ctx: &Context, arg: &ContainerIds) -> RpcResult<MultiResult> {

        info!("Called remove_containers({:?})", arg);

        let root = self.get_root(ctx).await?;

        let mut remove_errors = vec![];

        for cid in arg {
            let mut croot = root.clone();
            croot.push(cid);

            if let Err(e) = std::fs::remove_dir_all(&croot.as_path()) {
                if read_dir(&croot.as_path()).is_ok() {
                    remove_errors.push(
                        ItemResult {
                            error: Some(format!("{:?}", e.into_inner())),
                            key: cid.clone(),
                            success: true,
                        }
                    );
                }
            }
        }

        Ok(remove_errors)
    }
    
    /// Returns whether the object exists
    #[allow(unused)]
    async fn object_exists(&self, ctx: &Context, arg: &ContainerObject) -> RpcResult<bool> {
        info!("Called object_exists({:?})", arg);
        
        let root = self.get_root(ctx).await?;
        let file_path = Path::new(&root).join(&arg.container_id).join(&arg.object_id);

        match File::open(file_path) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false)
        }

    }
    
    /// Retrieves information about the object.
    /// Returns error if the object id is invalid or not found.
    #[allow(unused)]
    async fn get_object_info(
        &self,
        ctx: &Context,
        arg: &ContainerObject,
    ) -> RpcResult<ObjectMetadata> {

        let root = self.get_root(ctx).await?;
        let file_path = Path::new(&root).join(&arg.container_id).join(&arg.object_id);

        let metadata = metadata(file_path)?;

        let modified = match  metadata.modified()?.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(s) => Timestamp { sec: s.as_secs() as i64, nsec: 0u32 },
            Err(e) => return Err(RpcError::InvalidParameter(format!("{:?}", e))),
        };

        Ok(ObjectMetadata {
            container_id: arg.container_id.clone(),
            content_encoding: None,
            content_length: metadata.len() as u64,
            content_type: None,
            last_modified: Some(modified),
            object_id: arg.object_id.clone(),
        })
    }

    /// Lists the objects in the container.
    /// If the container exists and is empty, the returned `objects` list is empty.
    /// Parameters of the request may be used to limit the object names returned
    /// with an optional start value, end value, and maximum number of items.
    /// The provider may limit the number of items returned. If the list is truncated,
    /// the response contains a `continuation` token that may be submitted in
    /// a subsequent ListObjects request.
    ///
    /// Optional object metadata fields (i.e., `contentType` and `contentEncoding`) may not be
    /// filled in for ListObjects response. To get complete object metadata, use GetObjectInfo.
    /// Currently ignoring need for pagination
    #[allow(unused)]
    async fn list_objects(
        &self,
        ctx: &Context,
        arg: &ListObjectsRequest,
    ) -> RpcResult<ListObjectsResponse>{

        info!("Called list_objects({:?})", arg);
        
        let root = self.get_root(ctx).await?;
        let cdir = Path::new(&root).join(&arg.container_id);

        let mut objects = Vec::new();

        for entry in read_dir(&cdir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {

                let file_name = match entry.file_name().into_string() {
                    Ok(name) => name, 
                    Err(_) => {
                        return Err(RpcError::InvalidParameter(String::from("File name conversion failed")));
                    },
                };

                let modified = match  entry.metadata()?.modified()?.duration_since(SystemTime::UNIX_EPOCH) {
                    Ok(s) => Timestamp { sec: s.as_secs() as i64, nsec: 0u32 },
                    Err(e) => return Err(RpcError::InvalidParameter(format!("{:?}", e))),
                };

                objects.push(ObjectMetadata {
                    container_id: arg.container_id.clone(),
                    content_encoding: None,
                    content_length: entry.metadata()?.len(),
                    content_type: None,
                    last_modified: Some(modified),
                    object_id: file_name, 
                });
            }
        }

        Ok(ListObjectsResponse {
            continuation: None,
            is_last: true,
            objects,
        })
    }

    /// Removes the objects. In the event any of the objects cannot be removed,
    /// the operation continues until all requested deletions have been attempted.
    /// The MultiRequest includes a list of errors, one for each deletion request
    /// that did not succeed. If the list is empty, all removals succeeded.
    #[allow(unused)]
    async fn remove_objects(
        &self,
        ctx: &Context,
        arg: &RemoveObjectsRequest,
    ) -> RpcResult<MultiResult> {

        info!("Invoked remove obejcts: {:?}", arg);
        let root = self.get_root(ctx).await?;

        let mut errors = Vec::new();

        for object in &arg.objects {
            let opath = Path::join(
                &Path::join(&root,
                    &arg.container_id,
                    ),
            &object,
            );
            if let Err(e) = remove_file(opath.as_path()) {
                errors.push(ItemResult {
                    error: Some(format!("{:?}", e)),
                    key: format!("{:?}", opath),
                    success: false,
                })
            }
        }

        Ok(errors)

    }

    /// Requests to start upload of a file/blob to the Blobstore.
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    #[allow(unused)]
    async fn put_object(
        &self,
        ctx: &Context,
        arg: &PutObjectRequest,
    ) -> RpcResult<PutObjectResponse> {
        info!("Called put_object()");

        if arg.chunk.bytes.is_empty() {
            error!("put_object with zero bytes");
            return Err(RpcError::InvalidParameter(
                "cannot put zero-length objects".to_string(),
            ));
        }

        let stream_id = if arg.chunk.is_last {
            None 
        } else {
            Some(format!("{}+{}", arg.chunk.container_id, arg.chunk.object_id))
        };

        // store the chunks in order
        self.store_chunk(ctx, &arg.chunk, &stream_id).await?;

        Ok(PutObjectResponse { stream_id })

    }

    /// Uploads a file chunk to a blobstore. This must be called AFTER PutObject
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    #[allow(unused)]
    async fn put_chunk(&self, ctx: &Context, arg: &PutChunkRequest) -> RpcResult<()> {

        Err(RpcError::NotImplemented)

    }

    /// Requests to retrieve an object. If the object is large, the provider
    /// may split the response into multiple parts
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    #[allow(unused)]
    async fn get_object(
        &self,
        ctx: &Context,
        arg: &GetObjectRequest,
    ) -> RpcResult<GetObjectResponse> {

        info!("Get object called: {:?}", arg);

        let actor_id = self.get_actor_id(ctx).await?;

        let root = &self.get_root(ctx).await?;
        let cdir = Path::new(root).join(&arg.container_id);
        let file_path = Path::join(&cdir, &arg.object_id);

        let file = read(file_path)?;

        let c = Chunk {
            object_id: arg.object_id.clone(),
            container_id: arg.container_id.clone(),
            bytes: file,
            offset: 0,
            is_last: true,
        };

        info!("Read file {:?} size {:?}", arg.object_id, c.bytes.len());

        Ok(GetObjectResponse {
            content_encoding: None,
            content_length: c.bytes.len() as u64,
            content_type: None,
            error: None,
            initial_chunk: Some(c),
            success: true,
        })
    }

    fn contract_id() ->  & 'static str {  "wasmcloud:blobstore"}

}



