use serde_json::json;
use std::collections::HashMap;
use wasmbus_rpc::actor::prelude::*;
use wasmcloud_interface_httpserver::{HttpRequest, HttpResponse, HttpServer, HttpServerReceiver};
use wasmcloud_interface_logging::info;

#[allow(dead_code)]
pub mod wasmcloud_interface_blobstore {
    include!(concat!(env!("OUT_DIR"), "/gen/blobstore.rs"));
}
#[allow(dead_code)]
pub mod wasmcloud_interface_httpserver {
    include!(concat!(env!("OUT_DIR"), "/gen/httpserver.rs"));
}

#[derive(Debug, Default, Actor, HealthResponder)]
#[services(Actor, HttpServer)]
struct BlobviewActor {}

/// Implementation of HttpServer trait methods
#[async_trait]
impl HttpServer for BlobviewActor {
    /// Returns a greeting, "Hello World", in the response body.
    /// If the request contains a query parameter 'name=NAME', the
    /// response is changed to "Hello NAME"
    async fn handle_request(
        &self,
        _ctx: &Context,
        req: &HttpRequest,
    ) -> std::result::Result<HttpResponse, RpcError> {
        info!(
            "request: method:{}, url:{}, query:{}",
            &req.method, &req.path, &req.query_string
        );
        let method = req.method.to_ascii_uppercase();
        if method == "GET" && req.path == "/containers" {
            //let prov = BlobstoreSender::new();
            //let containers = prov.list_containers(ctx).await?;
            let body = json!( {
                "containers": [
                {
                "id": 1,
                "title": "ABC",
                "teaser": "Other",
                "body":
                r#"<p>Rerum velit quos est <strong>similique</strong>. Consectetur tempora eos ullam velit nobis sit debitis. Magni explicabo omnis delectus labore vel recusandae.</p><p>Aut a minus laboriosam harum placeat quas minima fuga. Quos nulla fuga quam officia tempore. Rerum occaecati ut eum et tempore. Nam ab repudiandae et nemo praesentium.</p><p>Cumque corporis officia occaecati ducimus sequi laborum omnis ut. Nam aspernatur veniam fugit. Nihil eum libero ea dolorum ducimus impedit sed. Quidem inventore porro corporis debitis eum in. Nesciunt unde est est qui nulla. Esse sunt placeat molestiae molestiae sed quia. Sunt qui quidem quos velit reprehenderit quos blanditiis ducimus. Sint et molestiae maxime ut consequatur minima. Quaerat rem voluptates voluptatem quos. Corporis perferendis in provident iure. Commodi odit exercitationem excepturi et deserunt qui.</p><p>Optio iste necessitatibus velit non. Neque sed occaecati culpa porro culpa. Quia quam in molestias ratione et necessitatibus consequatur. Est est tempora consequatur voluptatem vel. Mollitia tenetur non quis omnis perspiciatis deserunt sed necessitatibus. Ad rerum reiciendis sunt aspernatur.</p><p>Est ullam ut magni aspernatur. Eum et sed tempore modi.</p><p>Earum aperiam sit neque quo laborum suscipit unde. Expedita nostrum itaque non non adipisci. Ut delectus quis delectus est at sint. Iste hic qui ea eaque eaque sed id. Hic placeat rerum numquam id velit deleniti voluptatem. Illum adipisci voluptas adipisci ut alias. Earum exercitationem iste quidem eveniet aliquid hic reiciendis. Exercitationem est sunt in minima consequuntur. Aut quaerat libero dolorem.</p>"#,
                "#views": 143,
                "average_note": 2.72198,
                "commentable": true,
                "pictures": [],
                "published_at": "2012-08-06",
                                       "tags": [1, 3],
                                       "category": "tech",
                                       "subcategory": "computers",
                                       "backlinks": [
                                       {
                                           "date": "2012-08-09T00:00:00.000Z",
                                           "url": "http://example.com/bar/baz.html",
                                       },
                                       ],
                                       "notifications": [12, 31, 42],
                }
                ]}
            );
            let mut header = HashMap::new();
            header.insert(
                "content-type".to_string(),
                vec!["application/json".to_string()],
            );
            header.insert(
                "content-range".to_string(),
                vec!["containers 0-0/1".to_string()],
            );

            return Ok(HttpResponse {
                body: serde_json::to_vec(&body).unwrap(),
                header,
                status_code: 200,
            });
        }
        //let text = form_urlencoded::parse(req.query_string.as_bytes())
        //    .find(|(n, _)| n == "name")
        //    .map(|(_, v)| v.to_string())
        //    .unwrap_or_else(|| "World".to_string());

        Ok(HttpResponse {
            body: b"Hello ".to_vec(),
            header: Default::default(),
            status_code: 200,
        })
    }
}
