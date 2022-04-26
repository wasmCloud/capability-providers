use wasmbus_rpc::provider::prelude::*;
use wasmcloud_interface_blobstore::*;
use wasmcloud_test_util::{
    check,
    cli::print_test_results,
    provider_test::test_provider,
    testing::{TestOptions, TestResult},
};
#[allow(unused_imports)]
use wasmcloud_test_util::{run_selected, run_selected_spawn};

#[tokio::test]
async fn run_all() {
    let opts = TestOptions::default();
    let res = run_selected_spawn!(&opts,
        health_check,
        create_find_and_remove_dir,
        create_dirs_and_list,
        upload_and_list_files_in_dirs,
        upload_and_download_file,
        upload_download_chunked_file,
        );
    print_test_results(&res);

    let passed = res.iter().filter(|tr| tr.passed).count();
    let total = res.len();
    assert_eq!(passed, total, "{} passed out of {}", passed, total);

    // try to let the provider shut dowwn gracefully
    let provider = test_provider().await;
    let _ = provider.shutdown().await;
}

/// test that health check returns healthy
async fn health_check(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;

    // health check
    let hc = prov.health_check().await;
    check!(hc.is_ok())?;
    Ok(())
}

/// tests that you can create, find and remove directory (aka containters)
async fn create_find_and_remove_dir(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;

    // create client and ctx
    let client = BlobstoreSender::via(prov);
    let ctx = Context::default();

    let name: ContainerId = "container1".to_string();

    let resp = client.container_exists(&ctx, &name).await?;

    assert_eq!(resp, false);

    let resp2 = client.create_container(&ctx, &name).await;

    assert_eq!(resp2, Ok(()));

    let resp3 = client.container_exists(&ctx, &name).await?;

    assert_eq!(resp3, true);

    let conts: ContainerIds = vec![name.clone()];
    let resp4 = client.remove_containers(&ctx, &conts).await?;

    assert_eq!(resp4.len(), 0);

    let resp5 = client.container_exists(&ctx, &name).await?;

    assert_eq!(resp5, false);

    Ok(())
}

/// test that you can create directories (containers) and list them
async fn create_dirs_and_list(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;

    // create client and ctx
    let client = BlobstoreSender::via(prov);
    let ctx = Context::default();

    let mut resp = client.create_container(&ctx, &"cont1".into()).await;
    assert_eq!(resp, Ok(()));

    resp = client.create_container(&ctx, &"cont2".into()).await;
    assert_eq!(resp, Ok(()));

    resp = client.create_container(&ctx, &"cont2/cont3".into()).await;
    assert_eq!(resp, Ok(()));

    let resp2 = client.list_containers(&ctx).await?;
    assert_eq!(resp2.len(), 3);

    let conts: ContainerIds = vec!["cont1".into(), "cont2".into(), "cont2/cont3".into()];
    let resp3 = client.remove_containers(&ctx, &conts).await?;
    for c in &resp3 {
        println!("Could not remove {:?}", c);
    }
    assert_eq!(resp3.len(), 0);

    let resp4 = client.list_containers(&ctx).await?;
    assert_eq!(resp4.len(), 0);

    Ok(())
}


/// test that you can create objects (files) in directory (container) and list them
/// This test also checks most other operations on individual objects.
async fn upload_and_list_files_in_dirs(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;

    // create client and ctx
    let client = BlobstoreSender::via(prov);
    let mut ctx = Context::default();
    ctx.actor = Some("actor_test".into());

    // Create container
    let resp = client.create_container(&ctx, &"cont1".into()).await;
    assert_eq!(resp, Ok(()));

    // create and upload file1
    let file1_chunk = Chunk {
        object_id: "file1".into(),
        container_id: "cont1".into(),
        bytes: vec![0, 1, 2, 3, 4, 5],
        is_last: true,
        offset: 0,
    };
    let upload_request = PutObjectRequest {
        chunk: file1_chunk,
        content_encoding: None,
        content_type: None,
    };
    let mut resp2 = client.put_object(&ctx, &upload_request).await;
    assert_eq!(resp2, Ok(PutObjectResponse { stream_id: None }));

    // create and upload file2
    let file2_chunk = Chunk {
        object_id: "file2".into(),
        container_id: "cont1".into(),
        bytes: vec![6, 7, 8, 9, 10, 11],
        is_last: true,
        offset: 0,
    };
    let upload_request2 = PutObjectRequest {
        chunk: file2_chunk,
        content_encoding: None,
        content_type: None,
    };
    resp2 = client.put_object(&ctx, &upload_request2).await;
    assert_eq!(resp2, Ok(PutObjectResponse { stream_id: None }));

    // list objects (files) in container cont1
    let mut list_object_request = ListObjectsRequest::default();
    list_object_request.container_id = "cont1".to_string();
    let mut list_object_response = client.list_objects(&ctx, &list_object_request).await?;
    assert_eq!(list_object_response.is_last, true);
    assert_eq!(list_object_response.continuation, None);
    let objects = list_object_response.objects;
    assert_eq!(objects.len(), 2);

    let mut container_object = ContainerObject {
        container_id: "cont1".into(),
        object_id: "file2x".into(),
    };
    let mut exist = client.object_exists(&ctx, &container_object).await?;
    assert_eq!(exist, false);

    container_object = ContainerObject {
        container_id: "cont1".into(),
        object_id: "file2".into(),
    };
    exist = client.object_exists(&ctx, &container_object).await?;
    assert_eq!(exist, true);

    let object_info = client.get_object_info(&ctx, &container_object).await?;
    assert_eq!(object_info.container_id, "cont1".to_string());
    assert_eq!(object_info.content_length, 6);
    assert_eq!(object_info.object_id, "file2".to_string());

    let container_info = client.get_container_info(&ctx, &"cont1".to_string()).await?;
    assert_eq!(container_info.container_id, "cont1".to_string());
    assert_ne!(container_info.created_at, None);

    // remove the objects in the container
    let remove_object_request = RemoveObjectsRequest {
        container_id: "cont1".into(),
        objects: vec!["file1".into(), "file2".into()],
    };
    let remove_objects_response = client.remove_objects(&ctx, &remove_object_request).await?;
    println!("remove_objects: {:?}", remove_objects_response);
    assert_eq!(remove_objects_response.len(), 0);  // all objects should have been removed

    // Check that there are no more objects in container
    list_object_response = client.list_objects(&ctx, &list_object_request).await?;
    let objects = list_object_response.objects;
    assert_eq!(objects.len(), 0);

    // remove container (which now should be rmpty)
    let conts: ContainerIds = vec!["cont1".into()];
    let resp3 = client.remove_containers(&ctx, &conts).await?;
    assert_eq!(resp3.len(), 0);

    let resp4 = client.list_containers(&ctx).await?;
    assert_eq!(resp4.len(), 0);

    Ok(())
}


/// test that you can create objects (files) in directory (container) and list them
/// This test also checks most other operations on individual objects.
async fn upload_and_download_file(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;

    // create client and ctx
    let client = BlobstoreSender::via(prov);
    let mut ctx = Context::default();
    ctx.actor = Some("actor_test".into());

    // Create container
    let resp = client.create_container(&ctx, &"cont1".into()).await;
    assert_eq!(resp, Ok(()));

    // create and upload file1
    let file1_chunk = Chunk {
        object_id: "file1".into(),
        container_id: "cont1".into(),
        bytes: vec![0, 1, 2, 3, 4, 5],
        is_last: true,
        offset: 0,
    };
    let upload_request = PutObjectRequest {
        chunk: file1_chunk.clone(),
        content_encoding: None,
        content_type: None,
    };
    let resp2 = client.put_object(&ctx, &upload_request).await;
    assert_eq!(resp2, Ok(PutObjectResponse { stream_id: None }));

    // file is created. Now retrieve it again using get_object
    let get_object_request = GetObjectRequest {
        object_id: "file1".into(),
        container_id: "cont1".into(),
        range_start: Some(0),
        range_end: None,
    };
    let o = client.get_object(&ctx, &get_object_request).await?;
    assert_eq!(o.content_length, 6);
    assert_eq!(o.success, true);
    assert_ne!(o.initial_chunk, None);
    let c = o.initial_chunk.unwrap();
    assert_eq!(c.bytes, file1_chunk.bytes);

    // remove container (which now should be rmpty)
    let conts: ContainerIds = vec!["cont1".into()];
    let resp3 = client.remove_containers(&ctx, &conts).await?;
    assert_eq!(resp3.len(), 0);

    let resp4 = client.list_containers(&ctx).await?;
    assert_eq!(resp4.len(), 0);

    Ok(())
}


// test that you can upload a file larger than chunk size and download it again
async fn upload_chunked_download_file(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;

    // create client and ctx
    let client = BlobstoreSender::via(prov);
    let mut ctx = Context::default();
    ctx.actor = Some("actor_test".into());

    // Create container cont1
    let resp = client.create_container(&ctx, &"cont1".into()).await;
    assert_eq!(resp, Ok(()));

    // create and upload file1 part 1
    let file_chunk1 = Chunk {
        object_id: "file1".into(),
        container_id: "cont1".into(),
        bytes: vec![0, 1, 2, 3, 4, 5],
        is_last: false,
        offset: 0,
    };
    let upload_request = PutObjectRequest {
        chunk: file_chunk1.clone(),
        content_encoding: None,
        content_type: None,
    };
    let resp2 = client.put_object(&ctx, &upload_request).await;
    assert!(resp2.is_ok());

    let stream_id = match resp2 {
        Ok(po_response) => po_response.stream_id,
        Err(e) => return Err(RpcError::InvalidParameter(format!("{:?}", e))),
    };

    // create and upload file1 part 2
    let file_chunk2 = Chunk {
        object_id: "file1".into(),
        container_id: "cont1".into(),
        bytes: vec![10, 11, 12, 13, 14, 15, 16, 27],
        is_last: false,
        offset: 6,
    };
    let upload_2nd_chunk_request = PutChunkRequest {
        chunk: file_chunk2.clone(),
        stream_id: stream_id.clone(),
        cancel_and_remove: false,
    };
    let resp3 = client.put_chunk(&ctx, &upload_2nd_chunk_request).await;
    assert_eq!(resp3, Ok(()));

    // create and upload file1 part 3
    let file_chunk3 = Chunk {
        object_id: "file1".into(),
        container_id: "cont1".into(),
        bytes: vec![110, 111, 112, 113, 114, 115],
        is_last: true,
        offset: 14,
    };
    let upload_3rd_chunk_request = PutChunkRequest {
        chunk: file_chunk3.clone(),
        stream_id: stream_id,
        cancel_and_remove: false,
    };
    let resp4 = client.put_chunk(&ctx, &upload_3rd_chunk_request).await;
    assert_eq!(resp4, Ok(()));

    // file is created. Now retrieve it again using get_object assuming it will come back in one chunk
    let get_object_request = GetObjectRequest {
        object_id: "file1".into(),
        container_id: "cont1".into(),
        range_start: Some(0),
        range_end: None,
    };
    let o = client.get_object(&ctx, &get_object_request).await?;
    assert_eq!(o.content_length, 20);
    assert_eq!(o.success, true);
    assert_ne!(o.initial_chunk, None);
    let c = o.initial_chunk.unwrap();
    let mut combined = Vec::new();
    combined.append(&mut file_chunk1.bytes.clone());
    combined.append(&mut file_chunk2.bytes.clone());
    combined.append(&mut file_chunk3.bytes.clone());
    assert_eq!(c.bytes, combined);

    // remove container (which now should be rmpty)
    let conts: ContainerIds = vec!["cont1".into(), "cont2".into()];
    let resp5 = client.remove_containers(&ctx, &conts).await?;
    assert_eq!(resp5.len(), 0);

    let resp4 = client.list_containers(&ctx).await?;
    assert_eq!(resp4.len(), 0);

    Ok(())
}

// test that you can upload a file larger than chunk size and download it again
async fn upload_download_chunked_file(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;

    // create client and ctx
    let client = BlobstoreSender::via(prov);
    let mut ctx = Context::default();
    ctx.actor = Some("actor_test".into());

    // Create container cont1
    let resp = client.create_container(&ctx, &"cont1".into()).await;
    assert_eq!(resp, Ok(()));

    // create and upload file1 part 1
    let file_chunk1 = Chunk {
        object_id: "file1".into(),
        container_id: "cont1".into(),
        bytes: vec![0, 1, 2, 3, 4, 5, 10, 11, 12, 13, 14, 15, 16, 27, 110, 111, 112, 113, 114, 115],
        is_last: true,
        offset: 0,
    };
    let upload_request = PutObjectRequest {
        chunk: file_chunk1.clone(),
        content_encoding: None,
        content_type: None,
    };
    let resp2 = client.put_object(&ctx, &upload_request).await;
    assert!(resp2.is_ok());

    // file is created. Now retrieve it again using get_object assuming it will come back in one chunk
    let get_object_request = GetObjectRequest {
        object_id: "file1".into(),
        container_id: "cont1".into(),
        range_start: Some(0),
        range_end: None,
    };
    let o = client.get_object(&ctx, &get_object_request).await?;
    assert_eq!(o.content_length, 20);
    assert_eq!(o.success, true);
    assert_ne!(o.initial_chunk, None);
    let c = o.initial_chunk.unwrap();
    let mut combined = Vec::new();
    combined.append(&mut file_chunk1.bytes.clone());
    assert_eq!(c.bytes, combined);

    // remove container (which now should be rmpty)
    let conts: ContainerIds = vec!["cont1".into(), "cont2".into()];
    let resp5 = client.remove_containers(&ctx, &conts).await?;
    assert_eq!(resp5.len(), 0);

    let resp4 = client.list_containers(&ctx).await?;
    assert_eq!(resp4.len(), 0);

    Ok(())
}
