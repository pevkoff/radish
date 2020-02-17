use core::time::Duration;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[derive(Debug, Copy, Clone)]
struct Node<'a> {
    value: &'a str,
    ttl: Option<Duration>,
}

impl<'a> Node<'a> {
    fn new(value: &'a str, ttl: Option<Duration>) -> Self {
        Self {
            value: value,
            ttl: ttl,
        }
    }
}

#[derive(Debug, Clone)]
struct SafeMap<'a> {
    underlying: Arc<Mutex<HashMap<&'a str, Node<'a>>>>,
}

impl<'a> SafeMap<'a> {
    fn new(auto_evict: bool) -> Self {
        let map = Self {
            underlying: Arc::new(Mutex::new(HashMap::new())),
        };

        if !auto_evict {
            return map;
        }

        map
    }

    fn get(self, key: &'a str) -> Option<&'a str> {
        let map = self.underlying.lock().unwrap();
        match map.get(key) {
            Some(node) => Some(node.value),
            None => None,
        }
    }

    fn set(&self, key: &'a str, value: &'a str) -> Option<&'a str> {
        let node = Node::new(value, None);
        let mut map = self.underlying.lock().unwrap();

        map.insert(key, node);
        Some(value)
    }
}

#[derive(Serialize, Deserialize)]
struct GetRequest {
    key: String,
}
#[derive(Serialize, Deserialize)]
struct SetRequest {
    key: String,
    value: String,
}

async fn handler(req: Request<Body>, map: SafeMap<'_>) -> Result<Response<Body>, Infallible> {
    if req.uri().path() == "/" {
        return Ok(Response::new(Body::from("Index Page")));
    }

    match req.method() {
        &Method::GET => {
            let key = req.uri().path();

            match map.get(key) {
                Some(value) => println!("{}", value),
                None => println!("{}", "Nothing!"),
            }

            Ok(Response::new(Body::from("GET")))
        }
        &Method::POST => {
            match map.get("asd") {
                Some(value) => println!("{}", value),
                None => println!("{}", "Nothing!"),
            }

            Ok(Response::new(Body::from("GET")))
        }
        _ => Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::from("Method Not Allowed"))
            .unwrap()),
    }
}

#[tokio::main]
pub async fn main() {
    let safe_map = SafeMap::new(false);

    let service = make_service_fn(move |_| {
        let map = safe_map.clone();

        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                println!("{}", "Processing new req");
                handler(req, map.clone())
            }))
        }
    });

    let server = Server::bind(&SocketAddr::from(([0, 0, 0, 0], 5000))).serve(service);

    println!("Listening on 0.0.0.0:5000");
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
