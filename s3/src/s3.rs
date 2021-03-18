use crate::FileUpload;
use futures::TryStreamExt;
use hyper::{Client, Uri};
use hyper_proxy::{Intercept, Proxy, ProxyConnector};
use hyper_tls::HttpsConnector;
use log::info;
use rusoto_core::credential::{DefaultCredentialsProvider, StaticProvider};
use rusoto_core::Region;
use rusoto_s3::HeadObjectOutput;
use rusoto_s3::ListObjectsV2Output;
use rusoto_s3::Object;
use rusoto_s3::{
    CreateBucketRequest, DeleteBucketRequest, DeleteObjectRequest, GetObjectRequest,
    HeadObjectRequest, ListObjectsV2Request, PutObjectRequest, S3Client, S3,
};
use wasmcloud_actor_core::CapabilityConfiguration;

use std::error::Error;

type HttpConnector =
    hyper_proxy::ProxyConnector<hyper_tls::HttpsConnector<hyper::client::HttpConnector>>;

pub(crate) fn client_for_config(
    config: &CapabilityConfiguration,
) -> std::result::Result<S3Client, Box<dyn std::error::Error + Sync + Send>> {
    let region = if config.values.contains_key("REGION") {
        Region::Custom {
            name: config.values["REGION"].clone(),
            endpoint: if config.values.contains_key("ENDPOINT") {
                config.values["ENDPOINT"].clone()
            } else {
                "s3.us-east-1.amazonaws.com".to_string()
            },
        }
    } else {
        Region::UsEast1
    };

    let client = if config.values.contains_key("AWS_ACCESS_KEY") {
        info!("Creating provider from provided keys");
        let provider = StaticProvider::new(
            config.values["AWS_ACCESS_KEY"].to_string(),
            config.values["AWS_SECRET_ACCESS_KEY"].to_string(),
            config.values.get("AWS_TOKEN").cloned(),
            config
                .values
                .get("TOKEN_VALID_FOR")
                .map(|t| t.parse::<i64>().unwrap()),
        );
        let http_proxy = config.values["HTTP_PROXY"].to_string();
        let connector: HttpConnector = if http_proxy.is_empty() {
            ProxyConnector::new(HttpsConnector::new())?
        } else {
            info!("Proxy enabled for S3 client");
            let proxy = Proxy::new(Intercept::All, http_proxy.parse::<Uri>()?);
            ProxyConnector::from_proxy(hyper_tls::HttpsConnector::new(), proxy)?
        };
        let mut hyper_builder: hyper::client::Builder = Client::builder();
        // disabling due to connection closed issue
        hyper_builder.pool_max_idle_per_host(0);
        let client = rusoto_core::HttpClient::from_builder(hyper_builder, connector);
        S3Client::new_with(client, provider, region)
    } else {
        info!("Creating provider with default credentials");
        let provider = DefaultCredentialsProvider::new()?;
        S3Client::new_with(
            rusoto_core::request::HttpClient::new().expect("Failed to create HTTP client"),
            provider,
            region,
        )
    };

    Ok(client)
}

pub(crate) async fn create_bucket(
    client: &S3Client,
    name: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let create_bucket_req = CreateBucketRequest {
        bucket: name.to_string(),
        ..Default::default()
    };
    client.create_bucket(create_bucket_req).await?;
    Ok(())
}

pub(crate) async fn remove_bucket(
    client: &S3Client,
    bucket: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let delete_bucket_req = DeleteBucketRequest {
        bucket: bucket.to_owned(),
        ..Default::default()
    };

    client.delete_bucket(delete_bucket_req).await?;

    Ok(())
}

pub(crate) async fn remove_object(
    client: &S3Client,
    bucket: &str,
    id: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let delete_object_req = DeleteObjectRequest {
        bucket: bucket.to_string(),
        key: id.to_string(),
        ..Default::default()
    };

    client.delete_object(delete_object_req).await?;

    Ok(())
}

pub(crate) async fn get_blob_range(
    client: &S3Client,
    bucket: &str,
    id: &str,
    start: u64,
    end: u64,
) -> Result<Vec<u8>, Box<dyn Error + Sync + Send>> {
    let get_req = GetObjectRequest {
        bucket: bucket.to_owned(),
        key: id.to_owned(),
        range: Some(format!("bytes={}-{}", start, end)),
        ..Default::default()
    };

    let result = client.get_object(get_req).await?;
    let stream = result.body.unwrap();
    let body = stream
        .map_ok(|b| bytes::BytesMut::from(&b[..]))
        .try_concat()
        .await
        .unwrap();
    Ok(body.to_vec())
}

pub(crate) async fn complete_upload(
    client: &S3Client,
    upload: &FileUpload,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let bytes = upload
        .chunks
        .iter()
        .fold(vec![], |a, c| [&a[..], &c.chunk_bytes[..]].concat());
    let put_request = PutObjectRequest {
        bucket: upload.container.to_string(),
        key: upload.id.to_string(),
        body: Some(bytes.into()),
        ..Default::default()
    };

    log::info!("putting object");
    let res = client.put_object(put_request).await;
    log::info!("putting object: {:?}", res);
    res.unwrap();
    Ok(())
}

pub(crate) async fn list_objects(
    client: &S3Client,
    bucket: &str,
) -> Result<Option<Vec<Object>>, Box<dyn Error + Sync + Send>> {
    let list_obj_req = ListObjectsV2Request {
        bucket: bucket.to_owned(),
        ..Default::default()
    };
    let res: ListObjectsV2Output = client.list_objects_v2(list_obj_req).await?;

    Ok(res.contents)
}

pub(crate) async fn head_object(
    client: &S3Client,
    bucket: &str,
    key: &str,
) -> Result<HeadObjectOutput, Box<dyn Error + Sync + Send>> {
    let head_req = HeadObjectRequest {
        bucket: bucket.to_owned(),
        key: key.to_owned(),
        ..Default::default()
    };

    client.head_object(head_req).await.map_err(|e| e.into())
}
