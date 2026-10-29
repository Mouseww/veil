//! Instrumented real gateway for serial Codex/model evaluation; no synthetic responses.
use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use veil::proxy::{proxy_handler, AppState, UpstreamConfig};
use veil_engine::{
    creator::Creator,
    mapping::{Lookup, MappingStore, StoreError},
    placeholder::Placeholder,
};
use veil_store::SqliteStore;

#[derive(Clone)]
struct Context {
    output: Arc<PathBuf>,
    clock: Instant,
    sequence: Arc<AtomicU64>,
    current: Arc<AtomicU64>,
    mappings: Arc<Mutex<Vec<Value>>>,
    client: reqwest::Client,
}
impl Context {
    fn us(&self) -> u64 {
        self.clock.elapsed().as_micros() as u64
    }
    fn save(&self, name: &str, v: &Value) {
        fs::write(
            self.output.join(name),
            serde_json::to_vec_pretty(v).unwrap(),
        )
        .unwrap();
    }
}
struct AuditedStore {
    inner: SqliteStore,
    context: Context,
}
impl MappingStore for AuditedStore {
    fn get_or_insert(
        &self,
        creator: &Creator,
        prefix: &str,
        plaintext: &str,
    ) -> Result<Placeholder, StoreError> {
        let p = self.inner.get_or_insert(creator, prefix, plaintext)?;
        self.context.mappings.lock().unwrap().push(json!({"request_id":self.context.current.load(Ordering::SeqCst),"type":prefix,"plaintext":plaintext,"placeholder":p.format()}));
        Ok(p)
    }
    fn lookup(&self, creator: &Creator, p: &Placeholder) -> Result<Lookup, StoreError> {
        self.inner.lookup(creator, p)
    }
}
struct CaptureSink {
    context: Context,
    id: u64,
    side: &'static str,
    started: u64,
    status: u16,
    file: File,
    chunks: Vec<Value>,
    offset: usize,
    terminal: &'static str,
}
impl Drop for CaptureSink {
    fn drop(&mut self) {
        self.context.save(&format!("{:04}.{}.timing.json",self.id,self.side), &json!({"request_id":self.id,"side":self.side,"started_us":self.started,"ended_us":self.context.us(),"status":self.status,"response_bytes":self.offset,"chunks":self.chunks,"terminal":self.terminal}));
        if self.side == "client" {
            self.context.save(
                "mapping_audit.json",
                &json!(*self.context.mappings.lock().unwrap()),
            );
        }
    }
}
fn capture<S, E>(
    source: S,
    c: Context,
    id: u64,
    side: &'static str,
    started: u64,
    status: u16,
) -> Body
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    let file = File::create(c.output.join(format!("{id:04}.{side}.response.bin"))).unwrap();
    let sink = CaptureSink {
        context: c,
        id,
        side,
        started,
        status,
        file,
        chunks: Vec::new(),
        offset: 0,
        terminal: "consumer_dropped",
    };
    Body::from_stream(futures_util::stream::unfold(
        (Box::pin(source), sink),
        move |(mut stream, mut sink)| async move {
            match stream.next().await {
                Some(Ok(bytes)) => {
                    let at = sink.context.us();
                    sink.file.write_all(&bytes).unwrap();
                    sink.chunks
                        .push(json!({"at_us":at,"offset":sink.offset,"bytes":bytes.len()}));
                    sink.offset += bytes.len();
                    Some((Ok::<Bytes, std::io::Error>(bytes), (stream, sink)))
                }
                Some(Err(_)) => {
                    sink.terminal = "stream_error";
                    Some((
                        Err(std::io::Error::other("upstream stream error")),
                        (stream, sink),
                    ))
                }
                None => {
                    sink.terminal = "eof";
                    None
                }
            }
        },
    ))
}
fn safe_headers(headers: &axum::http::HeaderMap) -> Value {
    // Authorization, cookies, account IDs and all other header values are excluded.
    json!({"content_type":headers.get(header::CONTENT_TYPE).and_then(|h|h.to_str().ok()),"content_encoding":headers.get(header::CONTENT_ENCODING).and_then(|h|h.to_str().ok())})
}
async fn upstream(State(c): State<Context>, req: Request<Body>) -> Response {
    let started = c.us();
    let (parts, body) = req.into_parts();
    let id = parts
        .headers
        .get("x-veil-e2e-request-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let bytes = body.collect().await.unwrap().to_bytes();
    fs::write(
        c.output.join(format!("{id:04}.upstream.request.json")),
        &bytes,
    )
    .unwrap();
    c.save(&format!("{id:04}.upstream.request.meta.json"),&json!({"request_id":id,"started_us":started,"path":parts.uri.to_string(),"body_bytes":bytes.len(),"headers":safe_headers(&parts.headers),"target_host":"chatgpt.com"}));
    let mut headers = parts.headers;
    for h in [
        "host",
        "connection",
        "content-length",
        "transfer-encoding",
        "x-veil-e2e-request-id",
        "accept-encoding",
    ] {
        headers.remove(h);
    }
    headers.insert(header::ACCEPT_ENCODING, "identity".parse().unwrap());
    let response = c
        .client
        .request(parts.method, format!("https://chatgpt.com{}", parts.uri))
        .headers(headers)
        .body(bytes)
        .send()
        .await;
    let response = match response {
        Ok(r) => r,
        Err(e) => {
            c.save(
                &format!("{id:04}.network_error.json"),
                &json!({"connect":e.is_connect(),"timeout":e.is_timeout(),"at_us":c.us()}),
            );
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from("e2e upstream connection failed"))
                .unwrap();
        }
    };
    let status = response.status();
    c.save(&format!("{id:04}.upstream.headers.meta.json"),&json!({"request_id":id,"at_us":c.us(),"status":status.as_u16(),"headers":safe_headers(response.headers())}));
    let mut result = Response::builder().status(status);
    for (name, value) in response.headers() {
        if !matches!(
            name.as_str(),
            "transfer-encoding" | "content-length" | "connection" | "set-cookie"
        ) {
            result = result.header(name, value);
        }
    }
    result
        .body(capture(
            response.bytes_stream(),
            c,
            id,
            "upstream",
            started,
            status.as_u16(),
        ))
        .unwrap()
}
#[derive(Clone)]
struct Entry {
    context: Context,
    proxy: AppState,
}
async fn entry(State(e): State<Entry>, req: Request<Body>) -> Response {
    let c = e.context;
    if req.uri().path() == "/__e2e_health" {
        return Response::new(Body::from("ready"));
    }
    let started = c.us();
    let id = c.sequence.fetch_add(1, Ordering::SeqCst) + 1;
    c.current.store(id, Ordering::SeqCst);
    let (mut parts, body) = req.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    fs::write(
        c.output.join(format!("{id:04}.client.request.json")),
        &bytes,
    )
    .unwrap();
    c.save(&format!("{id:04}.client.request.meta.json"),&json!({"request_id":id,"started_us":started,"path":parts.uri.to_string(),"body_bytes":bytes.len(),"headers":safe_headers(&parts.headers)}));
    parts
        .headers
        .insert("x-veil-e2e-request-id", id.to_string().parse().unwrap());
    let response = proxy_handler(
        State(e.proxy),
        Request::from_parts(parts, Body::from(bytes)),
    )
    .await;
    let (parts, body) = response.into_parts();
    c.save(&format!("{id:04}.client.headers.meta.json"),&json!({"request_id":id,"at_us":c.us(),"status":parts.status.as_u16(),"headers":safe_headers(&parts.headers)}));
    let status = parts.status.as_u16();
    Response::from_parts(
        parts,
        capture(body.into_data_stream(), c, id, "client", started, status),
    )
}
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let output = PathBuf::from(&args[1]);
    fs::create_dir_all(&output).unwrap();
    let c = Context {
        output: Arc::new(output),
        clock: Instant::now(),
        sequence: Arc::new(AtomicU64::new(0)),
        current: Arc::new(AtomicU64::new(0)),
        mappings: Arc::new(Mutex::new(Vec::new())),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(150))
            .build()
            .unwrap(),
    };
    let store = Arc::new(AuditedStore {
        inner: SqliteStore::open(&args[2], &rand::random::<[u8; 32]>()).unwrap(),
        context: c.clone(),
    });
    let u = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let p = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut state = AppState::new(
        store,
        UpstreamConfig {
            anthropic_upstream: "http://127.0.0.1:1".into(),
            openai_completions_upstream: "http://127.0.0.1:1".into(),
            openai_responses_upstream: "http://127.0.0.1:1".into(),
        },
        32 * 1024 * 1024,
    );
    // The production subscription path goes through this diagnostic recorder to the real model.
    state.client = reqwest::Client::builder().no_proxy().build().unwrap();
    state.codex_chatgpt_upstream = format!(
        "http://127.0.0.1:{}/backend-api/codex",
        u.local_addr().unwrap().port()
    );
    c.save("ready.json",&json!({"proxy_port":p.local_addr().unwrap().port(),"upstream_recorder_port":u.local_addr().unwrap().port(),"unix_ms":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),"pid":std::process::id(),"real_upstream":"https://chatgpt.com/backend-api/codex","store":"SqliteStore","rules":"builtin_default"}));
    let a = Router::new().fallback(any(upstream)).with_state(c.clone());
    let b = Router::new().fallback(any(entry)).with_state(Entry {
        context: c,
        proxy: state,
    });
    tokio::select! { r=axum::serve(u,a)=>r.unwrap(),r=axum::serve(p,b)=>r.unwrap() }
}
