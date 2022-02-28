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
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    fs::{read, read_dir, remove_file},
};
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
struct FsProviderStatus {
    root: PathBuf,
    upload_chunks: HashMap<ChunkOffsetKey, Chunk>,
    download_chunks: HashMap<ChunkOffsetKey, Chunk>,
}


/// fs capability provider implementation
#[derive(Default, Clone, Provider)]
#[services(Blobstore)]
struct FsProvider {
    /// The actors field 
    actors: Arc<RwLock<HashMap<String, FsProviderStatus>>>,
}

impl FsProvider {

    /// Get actor id string based on context value
    fn get_actor_id(ctx: &Context) -> RpcResult<String> {
        let actor_id = match &ctx.actor {
            Some(id) => sanitize_id(id),
            None => {
                return Err(RpcError::InvalidParameter(String::from("No actor id found")));
            },
        };
        Ok(actor_id)
    }

    /// Get the provider configuration set by the link definition.
    async fn get_config(&self, ctx: &Context) -> RpcResult<FsProviderStatus> {
        let actor_id = FsProvider::get_actor_id(ctx)?;
        let config_map = self.actors.read().await;
        let actor_config = match config_map.get(&actor_id) {
            Some(config) => config,
            None => {
                return Err(RpcError::InvalidParameter(String::from("No root configuration found")));
            },  
        };
        Ok(actor_config.clone())
    }

    /// Stores a file chunk in right order.
    fn store_chunk(chunk: &Chunk, root: &Path, _upload_chunks: &mut HashMap<ChunkOffsetKey, Chunk>) -> RpcResult<Option<String>> {

        let _key = format!("{}+{}", chunk.container_id, chunk.object_id);
    
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

        Ok(None)
    
    }

}


/// use default implementations of provider message handlers
impl ProviderDispatch for FsProvider {}

#[async_trait]
impl ProviderHandler for FsProvider {
    /// The fs provider has one configuration parameter, the root of the file system
    /// 
    async fn put_link(&self, ld: &LinkDefinition) -> RpcResult<bool> {

        let mut config_map = self.actors.write().await;
        let actor_id = ld.actor_id.to_string();

        let mut  root = PathBuf::from(match ld.values.get("ROOT") {
            Some(v) => Path::new(v),
            None => Path::new("/tmp"),     // perhaps not portable across different systems
        });

        root.push(actor_id.clone());

        info!("File System Blob Store Container Root: '{:?}'", &root);

        let config = FsProviderStatus {
            root,
            upload_chunks: HashMap::new(),
            download_chunks: HashMap::new(),
        };

        config_map.insert(actor_id, config);

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
        
        let root = self.get_config(ctx).await?.root;
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
        let root = self.get_config(ctx).await?.root;
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
        Ok(ContainerMetadata {
            container_id: "name".to_string(),
            created_at: None,
        })
    }

    /// Returns list of container ids
    #[allow(unused)]
    async fn list_containers(&self, ctx: &Context) -> RpcResult<ContainersInfo> {

        let root = self.get_config(ctx).await?.root;

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

        let root = self.get_config(ctx).await?.root;

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
        Ok(true)
    }
    
    /// Retrieves information about the object.
    /// Returns error if the object id is invalid or not found.
    #[allow(unused)]
    async fn get_object_info(
        &self,
        ctx: &Context,
        arg: &ContainerObject,
    ) -> RpcResult<ObjectMetadata> {
        Ok(ObjectMetadata {
            container_id: "name".to_string(),
            content_encoding: None,
            content_length: 0,
            content_type: None,
            last_modified: None,
            object_id: "object_name".to_string(),
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
        
        let root = self.get_config(ctx).await?.root;
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
        Ok(vec![ItemResult {
            error: None,
            key: "key".to_string(),
            success: true,
        }])
    }

    /// Requests to start upload of a file/blob to the Blobstore.
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    #[allow(unused)]
    async fn put_object(
        &self,
        ctx: &Context,
        arg: &PutObjectRequest,
    ) -> RpcResult<PutObjectResponse> {
        info!("Called put_object({:?})", arg.chunk);

        if !arg.chunk.is_last {
            error!("put_object for multi-part upload: not implemented!");
            return Err(RpcError::InvalidParameter(
                "multipart upload not implemented".to_string(),
            ));
        }
        if arg.chunk.offset != 0 {
            error!("put_object with initial offset non-zero: not implemented!");
            return Err(RpcError::InvalidParameter(
                "non-zero offset not supported".to_string(),
            ));
        }
        if arg.chunk.bytes.is_empty() {
            error!("put_object with zero bytes");
            return Err(RpcError::InvalidParameter(
                "cannot put zero-length objects".to_string(),
            ));
        }
    
        let actor_id = FsProvider::get_actor_id(ctx)?;
        let mut config_map = self.actors.write().await;
        let config = match config_map.get_mut(&actor_id) {
            Some(config) => config,
            None => {
                return Err(RpcError::InvalidParameter(String::from("No root configuration found")));
            },
        };
        
        let cdir = Path::new(&config.root).join(&arg.chunk.container_id);
        let bfile = Path::join(&cdir, &arg.chunk.object_id);

        // create an empty file
        let resp = std::fs::write(&bfile, &[]);

        if resp.is_err() {
            let error_string = format!("Could not create file: {:?}", bfile).to_string();
            error!("{:?}", &error_string);
            return Err(RpcError::InvalidParameter(error_string));
        }

        // store the chunks in order
        let stream_id = FsProvider::store_chunk(&arg.chunk, &config.root, & mut config.upload_chunks)?;

        Ok(PutObjectResponse { stream_id })

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
        Ok(GetObjectResponse {
            content_encoding: None,
            content_length: 0,
            content_type: None,
            error: None,
            initial_chunk: None,
            success: true,
        })
    }

    /// Uploads a file chunk to a blobstore. This must be called AFTER PutObject
    /// It is recommended to keep chunks under 1MB to avoid exceeding nats default message size
    #[allow(unused)]
    async fn put_chunk(&self, ctx: &Context, arg: &PutChunkRequest) -> RpcResult<()> {
        Ok(())
    }
    

/*  
   
    /// Remove the objects.
    /// The objects do not need to be in the same container
    async fn remove_objects(&self, ctx: &Context, arg: &ObjectList) -> RpcResult<BlobstoreResult> {

        info!("Invoked remove obejcts: {:?}", arg);
        let root = self.get_config(ctx).await?.root; 

        for object in arg {
            let opath = Path::join(
                &Path::join(&root,
                    sanitize_id(&object.container_id),
                    ),
            sanitize_id(&object.id),
            );
            remove_file(opath.as_path())?;
        }
        Ok(BlobstoreResult {success: true, error: None})
    }

   
    /// Requests to start a download of a file/blob from the Blobstore
    /// It is recommended to keep chunks under 1MB to not exceed wasm memory allocation
    async fn start_download(
        &self,
        ctx: &Context,
        arg: &DownloadChunkArgs,
    ) -> RpcResult<DownloadResult> {

        info!("Called start_download({:?})", arg);

        let actor_id = FsProvider::get_actor_id(ctx)?;
        let mut config_map = self.actors.write().await;
        let config = match config_map.get_mut(&actor_id) {
            Some(config) => config,
            None => {
                return Err(RpcError::InvalidParameter(String::from("No root configuration found")));
            },
        };

        let obj_file = Path::join(
            &Path::join(
                &config.root,
                sanitize_id(&arg.object_metadata.container_id),
            ),
            sanitize_id(&arg.object_metadata.id),
        );

        let file = read(obj_file)?;

        let mut chunks = file.chunks(arg.chunk_size as usize);
        let first_chunk_slice = chunks.next().unwrap(); 

        // if there is more than one chunk, store them in download_chunks with this key
        let key = sanitize_id(&arg.object_metadata.container_id) + &sanitize_id(&arg.object_metadata.id);
    
        if chunks.len() > 1 {
            let mut sequence_number = 1u64;
            for chunk_slice in chunks {
                 let chunk = Chunk {
                    bytes: Vec::from(chunk_slice),
                    chunk_size: arg.chunk_size as u64,
                    object_data: arg.object_metadata.clone(),
                    sequence_no: sequence_number,
                };
                config.download_chunks.insert((key.clone(), sequence_number), chunk);
                sequence_number += 1;
            }
        }

        let chunk0 = Chunk {
            bytes: Vec::from(first_chunk_slice),
            chunk_size: arg.chunk_size as u64,
            object_data: arg.object_metadata.clone(),
            sequence_no: 0,
        };

        Ok(DownloadResult { success: true, error: None, chunk: Some(chunk0),})
    }

    #[allow(unused)]
    /// Receives a file chunk from a blobstore. This must be called AFTER
    /// the StartDownload operation.
    /// It is recommended to keep chunks under 1MB to not exceed wasm memory allocation
    async fn receive_chunk(&self, ctx: &Context, arg: &DownloadChunkArgs) -> RpcResult<DownloadResult> {
        info!("Called receive_chunk({:?})", arg);

        let actor_id = FsProvider::get_actor_id(ctx)?;
        let mut config_map = self.actors.write().await;
        let config = match config_map.get_mut(&actor_id) {
            Some(config) => config,
            None => {
                return Err(RpcError::InvalidParameter(String::from("No root configuration found")));
            },
        };

        // Get the next chunk in sequence
        let key = sanitize_id(&arg.object_metadata.container_id) + &sanitize_id(&arg.object_metadata.id);
        let next_chunk = config.download_chunks.get(&(key.clone(), arg.sequence_number));

        match next_chunk {
            Some(chunk) => Ok(DownloadResult { success: true, error: None, chunk: Some(chunk.clone()),}),
            None =>  Ok(DownloadResult { 
                                        success: false, 
                                        error: Some(format!("Chunk for {:?} not found", arg)), 
                                        chunk: None,
                                    }),
        }
    }   

 */
    fn contract_id() ->  & 'static str {  "wasmcloud:blobstore"}

}


#[allow(dead_code)]
fn sanitize_id(id: &str) -> String {
    let bad_prefixes: &[_] = &['/', '.'];
    let s = id.trim_start_matches(bad_prefixes);
    let s = s.replace("..", "");
    s.replace("/", "_")
}

