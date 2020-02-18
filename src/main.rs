use bytes::buf::BufExt;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

fn get_timestamp() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn calc_expiration(ttl: Duration) -> u128 {
    get_timestamp() + ttl.as_millis()
}

#[derive(Debug, Clone)]
struct Node {
    value: String,
    expiration: Option<u128>,
}

impl Node {
    fn new(value: String, expiration: Option<u128>) -> Self {
        Self {
            value: value,
            expiration: expiration,
        }
    }
}

#[derive(Debug, Clone)]
struct SafeMap {
    underlying: Arc<Mutex<HashMap<String, Node>>>,
    auto_evict: bool,
}

impl SafeMap {
    fn new(auto_evict: bool) -> Self {
        Self {
            underlying: Arc::new(Mutex::new(HashMap::new())),
            auto_evict: auto_evict,
        }
    }

    fn get(self, key: String) -> Option<String> {
        let mut map = self.underlying.lock().unwrap();
        let key = key.as_str();

        match map.get(key) {
            Some(node) => {
                if let Some(expiration) = node.expiration {
                    if get_timestamp() > expiration {
                        map.remove(key);
                        return None;
                    }
                }
                Some(node.value.clone())
            }
            _ => None,
        }
    }

    fn set(self, key: String, value: String, ttl: Option<Duration>) -> Option<String> {
        let node = Node::new(
            value.clone(),
            ttl.map_or(None, |ttl| Some(calc_expiration(ttl))),
        );
        let mut map = self.underlying.lock().unwrap();

        map.insert(key, node);
        Some(value)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct RadishResponse {
    query: String,
    data: Option<String>,
}

impl RadishResponse {
    fn new(query: String, data: Option<String>) -> Self {
        Self {
            query: query,
            data: data,
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
struct RadishSetRequest {
    value: String,
    ttl: u64,
}

async fn handler(req: Request<Body>, map: SafeMap) -> Result<Response<Body>, Infallible> {
    if req.uri().path() == "/" {
        return Ok(Response::new(Body::from("Index Page")));
    }
    if req.method() != &Method::GET && req.method() != &Method::POST {
        return Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::from("Method Not Allowed"))
            .unwrap());
    }

    let mut path = req.uri().path().to_string();
    path.remove(0);
    let key = path;

    let data = match req.method() {
        &Method::GET => map.get(key.clone()),
        &Method::POST => {
            let body = (hyper::body::aggregate(req).await).unwrap();
            let r: RadishSetRequest = serde_json::from_reader(body.reader()).unwrap();
            map.set(key.clone(), r.value, Some(Duration::from_secs(r.ttl)))
        }
        _ => None,
    };
    let response = RadishResponse::new(key.clone(), data);
    let serialized = serde_json::to_string(&response).unwrap();

    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .status(StatusCode::OK)
        .body(Body::from(serialized))
        .unwrap())
}

#[tokio::main]
pub async fn main() {
    let safe_map = SafeMap::new(true);

    // Это надо перенести в функцию SafeMap
    let cln = safe_map.clone();
    tokio::spawn(async move {
        loop {
            let map = cln.clone();
            tokio::time::delay_for(Duration::from_secs(5)).await;

            let now = get_timestamp();
            map.underlying
                .lock()
                .unwrap()
                .retain(|_, v| v.expiration.map_or(true, |exp| exp > now));
        }
    });

    let service = make_service_fn(move |_| {
        let map = safe_map.clone();
        async move { Ok::<_, Infallible>(service_fn(move |req| handler(req, map.clone()))) }
    });

    let server = Server::bind(&SocketAddr::from(([0, 0, 0, 0], 5000))).serve(service);

    println!("Listening on 0.0.0.0:5000");
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
