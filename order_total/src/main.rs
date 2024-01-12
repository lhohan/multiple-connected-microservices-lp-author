#[macro_use]
extern crate lazy_static;

use std::net::SocketAddr;
use std::convert::Infallible;
use std::str;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode, Server};
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref SALES_TAX_RATE_SERVICE: String = {
        if let Ok(url) = std::env::var("SALES_TAX_RATE_SERVICE") {
            url
        } else {
            "http://localhost:8001/find_rate".into()
        }
    };
}

#[derive(Serialize, Deserialize, Debug)]
struct Order {
    order_id: i32,
    product_id: i32,
    quantity: i32,
    subtotal: f32,
    shipping_address: String,
    shipping_zip: String,
    total: f32,
}

/*
impl Order {
    fn new(
        order_id: i32,
        product_id: i32,
        quantity: i32,
        subtotal: f32,
        shipping_address: String,
        shipping_zip: String,
        total: f32,
    ) -> Self {
        Self {
            order_id,
            product_id,
            quantity,
            subtotal,
            shipping_address,
            shipping_zip,
            total,
        }
    }
}
*/

/// This is our service handler. It receives a Request, routes on its
/// path, and returns a Future of a Response.
async fn handle_request(req: Request<Body>) -> Result<Response<Body>, anyhow::Error> {
    match (req.method(), req.uri().path()) {
        // CORS OPTIONS
        (&Method::OPTIONS, "/compute") => Ok(response_build(&String::from(""))),

        // Serve some instructions at /
        (&Method::GET, "/") => Ok(Response::new(Body::from(
            "Try POSTing data to /compute such as: `curl localhost:8002/compute -XPOST -d '...'`",
        ))),

        (&Method::POST, "/compute") => {
            let byte_stream = hyper::body::to_bytes(req).await?;
            let mut order: Order = serde_json::from_slice(&byte_stream).unwrap();

            let client = reqwest::Client::new();
            let result = client.post(&*SALES_TAX_RATE_SERVICE)
                .body(order.shipping_zip.clone())
                .send()
                .await;
            let mapped_result = result.as_ref().map(|response| response.status().as_u16());
            match mapped_result {
                Ok(200) => {
                    let rate = result.unwrap()
                        .text()
                        .await?
                        .parse::<f32>()?;

                    order.total = order.subtotal * (1.0 + rate);
                    Ok(response_build(&serde_json::to_string_pretty(&order)?))
                },
                _ => {
                    let err_message = format!("{{\"status\":\"error\", \"message\":\"The zip code ({}) in the order does not have a corresponding sales tax rate.\"}}", order.shipping_zip.clone());
                    Ok(response_build(err_message.as_str()))
                }
            }
        }

        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

// CORS headers
fn response_build(body: &str) -> Response<Body> {
    Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header("Access-Control-Allow-Headers", "api,Keep-Alive,User-Agent,Content-Type")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8002));
    let make_svc = make_service_fn(|_| {
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(req)
            }))
        }
    });
    let server = Server::bind(&addr).serve(make_svc);
    dbg!("Server started on port 8002");
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
    Ok(())
}
