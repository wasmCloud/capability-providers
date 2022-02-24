//! Tests kv-vault
//!
use kv_vault_lib::wasmcloud_interface_keyvalue::*;
use wasmbus_rpc::{provider::prelude::Context, RpcResult};
use wasmcloud_test_util::{
    check, check_eq,
    cli::print_test_results,
    provider_test::{test_provider, Provider},
    run_selected_spawn,
    testing::{TestOptions, TestResult},
};

#[tokio::test]
async fn run_all() {
    let opts = TestOptions::default();
    let res = run_selected_spawn!(&opts, health_check, get_set, contains_del,);
    print_test_results(&res);

    let passed = res.iter().filter(|tr| tr.passed).count();
    let total = res.len();
    assert_eq!(passed, total, "{} passed out of {}", passed, total);

    // try to let the provider shut down gracefully
    let provider = test_provider().await;
    let _ = provider.shutdown().await;
}

/// returns a new test key with the given prefix
/// The purpose is to make sure different tests don't collide with each other
fn new_key(prefix: &str) -> String {
    format!("{}_{:x}", prefix, rand::random::<u32>())
}

// syntactic sugar for set
async fn set<T1: ToString, T2: ToString>(
    kv: &KeyValueSender<Provider>,
    ctx: &Context,
    key: T1,
    value: T2,
) -> RpcResult<()> {
    kv.set(
        ctx,
        &SetRequest {
            key: key.to_string(),
            value: value.to_string(),
            ..Default::default()
        },
    )
    .await
}

/// test that health check returns healthy
async fn health_check(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;

    // health check
    let hc = prov.health_check().await;
    check!(hc.is_ok())?;
    Ok(())
}

/// get and set
async fn get_set(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;
    env_logger::try_init().ok();

    // create client and ctx
    let kv = KeyValueSender::via(prov);
    let ctx = Context::default();

    let list = kv.set_query(&ctx, "test_get").await?;
    assert_eq!(list.len(), 0, "nothing before write");

    let key = new_key("test_get/get");
    const VALUE: &str = "Alice";

    let get_resp = kv.get(&ctx, &key).await.expect("get non-existent key");
    check_eq!(get_resp.exists, false)?;

    set(&kv, &ctx, &key, &VALUE.to_string())
        .await
        .expect("set key first time");

    let get_resp = kv.get(&ctx, &key).await.expect("get exists");
    check!(get_resp.exists).expect("get exists check");
    check_eq!(get_resp.value.as_str(), VALUE).expect("get exists value check");

    let list = kv.set_query(&ctx, "test_get").await?;
    assert_eq!(list.len(), 1, "list after first set");

    // clean up
    //let _ = kv.del(&ctx, &key).await.expect("delete key");
    Ok(())
}

/// contains and del
async fn contains_del(_opt: &TestOptions) -> RpcResult<()> {
    let prov = test_provider().await;

    // create client and ctx
    let kv = KeyValueSender::via(prov);
    let ctx = Context::default();

    let key = new_key("test_cdel/contains");
    const VALUE: &str = "Bob";

    let has_key_before_set = kv.contains(&ctx, &key).await?;
    check_eq!(has_key_before_set, false)?;

    set(&kv, &ctx, &key, VALUE).await?;

    let has_key_after_set = kv.contains(&ctx, &key).await?;

    check_eq!(has_key_after_set, true)?;

    // clean up
    let _ = kv.del(&ctx, &key).await?;
    let has_key_after_del = kv.contains(&ctx, &key).await?;
    check_eq!(has_key_after_del, false)?;

    Ok(())
}
