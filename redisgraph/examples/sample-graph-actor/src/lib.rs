// Copyright 2015-2020 Capital One Services, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// **
// This sample actor responds to incoming HTTP requests.
// GET - queries the graph and returns two values from a strongly-typed tuple
// POST - creates the data to be used for the query. Note this isn't idempotent, you will
//        grow your graph DB each time this is executed.
//
// The Cypher queries for this demo come from https://github.com/malte-v/redisgraph-rs
// **

// extern crate wascc_actor as actor;
// use actor::prelude::*;
// use wasccgraph_actor::graph;

extern crate actor_graphdb;
extern crate wapc_guest as guest;
use actor_http_server as http;
use guest::prelude::*;

#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate log;

#[no_mangle]
pub fn wapc_init() {
    http::Handlers::register_handle_request(handle_http_request);
    actor_core::Handlers::register_health_request(health);
}

fn handle_http_request(req: http::Request) -> HandlerResult<http::Response> {
    info!("Handling HTTP request"); // requires wasmcloud:logging
    if req.method.to_uppercase() == "POST" {
        create_data()
    } else {
        query_data()
    }
}

// Execute a Cypher query to add data
fn create_data() -> HandlerResult<http::Response> {
    info!("Creating graph data");
    actor_graphdb::graph::default().graph("MotoGP").mutate("CREATE (:Rider {name: 'Valentino Rossi', birth_year: 1979})-[:rides]->(:Team {name: 'Yamaha'}), \
    (:Rider {name:'Dani Pedrosa', birth_year: 1985, height: 1.58})-[:rides]->(:Team {name: 'Honda'}), \
    (:Rider {name:'Andrea Dovizioso', birth_year: 1986, height: 1.67})-[:rides]->(:Team {name: 'Ducati'})")?;

    Ok(http::Response::ok())
}

// Execute a Cypher query to return data values
fn query_data() -> HandlerResult<http::Response> {
    info!("Querying graph data");
    let (name, birth_year): (String, u32) = actor_graphdb::graph::default().graph("MotoGP").query(
        "MATCH (r:Rider)-[:rides]->(t:Team) WHERE t.name = 'Yamaha' RETURN r.name, r.birth_year",
    )?;

    let result = json!({
        "name": name,
        "birth_year": birth_year
    });
    Ok(http::Response::json(result, 200, "OK"))
}

fn health(_req: actor_core::HealthCheckRequest) -> HandlerResult<actor_core::HealthCheckResponse> {
    Ok(actor_core::HealthCheckResponse::healthy())
}
