use anyhow::{anyhow, Result};
use hyper::{
    body,
    http::{header, Method, StatusCode},
    server::conn::http1,
    service::service_fn,
    Request, Response,
};
use hyper_util::rt::TokioIo;
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Api, ListParams},
    Client,
};
use log::{error, info};
use prometheus_client::{
    encoding::text::encode, encoding::EncodeLabelSet, metrics::family::Family,
    metrics::gauge::Gauge, registry::Registry,
};
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;
use std::{env, fs, io::Read, path::Path};
use tokio::net::TcpListener;
use url::Url;

const SERVICE_ACCOUNT_TOKEN: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";
const CLIENT_CERT_BUNDLE: &str = "/var/run/secrets/kubernetes.io/serviceaccount/ca.crt";

// NodeStats represents stats for a single node. It holds the deserialized
// JSON response from the Kubernetes API
#[derive(Deserialize, Debug)]
struct NodeStats {
    node: NodeSummary,
    pods: Vec<Pod>,
}

#[derive(Deserialize, Debug)]
struct NodeSummary {
    #[serde(rename = "nodeName")]
    name: String,
}

// Pod represents a single pod. It holds the pod's metadata and ephemeral
// storage stats
#[derive(Deserialize, Debug)]
struct Pod {
    #[serde(rename = "podRef")]
    pod_ref: PodRef,
    #[serde(rename = "ephemeral-storage")]
    ephemeral_storage: EphemeralStorage,
}

#[derive(Deserialize, Debug)]
struct PodRef {
    name: String,
    namespace: String,
    uid: String,
}

#[derive(Deserialize, Debug)]
struct EphemeralStorage {
    #[serde(rename = "availableBytes")]
    available_bytes: i64,
    #[serde(rename = "capacityBytes")]
    capacity_bytes: i64,
    #[serde(default, rename = "usedBytes")]
    used_bytes: i64,
}

#[derive(Default)]
struct EphemeralStorageMetrics {
    available_bytes: Family<EphemeralStorageLabels, Gauge>,
    capacity_bytes: Family<EphemeralStorageLabels, Gauge>,
    used_bytes: Family<EphemeralStorageLabels, Gauge>,
}

// EphemeralStorage metrics label set
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, EncodeLabelSet)]
struct EphemeralStorageLabels {
    pod: String,
    namespace: String,
    node: String,
    uid: String,
}

#[derive(Default)]
struct BuildInfoMetrics {
    build_info: Family<BuildInfoLabels, Gauge>,
}

// BuildInfoMetrics metric label set
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, EncodeLabelSet)]
struct BuildInfoLabels {
    version: String,
    revision: String,
}

impl EphemeralStorageMetrics {
    fn register(&mut self, registry: &mut Registry) {
        registry.register(
            "kube_pod_ephemeral_storage_available_bytes",
            "Pod ephemeral storage available bytes",
            self.available_bytes.clone(),
        );
        registry.register(
            "kube_pod_ephemeral_storage_capacity_bytes",
            "Pod ephemeral storage capacity bytes",
            self.capacity_bytes.clone(),
        );
        registry.register(
            "kube_pod_ephemeral_storage_used_bytes",
            "Pod ephemeral storage used bytes",
            self.used_bytes.clone(),
        );
    }
}

impl BuildInfoMetrics {
    fn register(&mut self, registry: &mut Registry) {
        registry.register(
            "kube_stats_exporter_build_info",
            "kube-stats-exporter build info",
            self.build_info.clone(),
        );
    }
}

// This function sends a request to the Kubernetes API and to get the node
// stats and returns the ephemeral storage metrics
async fn get_stats() -> Result<String> {
    // create a Client to connect to the Kubernetes cluster
    let client = Client::try_default().await?;
    let nodes: kube::Api<Node> = Api::all(client);

    let mut registry = Registry::default();
    let mut storage_metrics = EphemeralStorageMetrics::default();
    storage_metrics.register(&mut registry);

    let mut info_metrics = BuildInfoMetrics::default();
    info_metrics.register(&mut registry);

    info_metrics
        .build_info
        .get_or_create(&BuildInfoLabels {
            version: env!("CARGO_PKG_VERSION").to_string(),
            revision: env!("GIT_HASH").to_string(),
        })
        .set(1);

    let mut tasks = vec![];

    // iterate over the nodes and get the stats
    for node in nodes.list(&ListParams::default().labels("")).await? {
        match get_node_name(&node) {
            Ok(node_name) => {
                // spawn returns a JoinHandler
                tasks.push(tokio::spawn(async move {
                    get_node_stats(node_name.clone()).await
                }));
            }
            Err(err) => {
                error!("Error getting node name for {node:?}: {err}");
                continue;
            }
        }
    }

    for task in tasks {
        let stats = match task.await? {
            Ok(stats) => stats,
            Err(err) => {
                error!("{}", err);
                continue;
            }
        };

        // populate the Labels
        for pod in stats.pods.into_iter() {
            storage_metrics
                .used_bytes
                .get_or_create(&EphemeralStorageLabels {
                    pod: pod.pod_ref.name.clone(),
                    namespace: pod.pod_ref.namespace.clone(),
                    node: stats.node.name.clone(),
                    uid: pod.pod_ref.uid.clone(),
                })
                .set(pod.ephemeral_storage.used_bytes);
            storage_metrics
                .available_bytes
                .get_or_create(&EphemeralStorageLabels {
                    pod: pod.pod_ref.name.clone(),
                    namespace: pod.pod_ref.namespace.clone(),
                    node: stats.node.name.clone(),
                    uid: pod.pod_ref.uid.clone(),
                })
                .set(pod.ephemeral_storage.available_bytes);
            storage_metrics
                .capacity_bytes
                .get_or_create(&EphemeralStorageLabels {
                    pod: pod.pod_ref.name,
                    namespace: pod.pod_ref.namespace,
                    node: stats.node.name.clone(),
                    uid: pod.pod_ref.uid,
                })
                .set(pod.ephemeral_storage.capacity_bytes);
        }
    }

    let mut buffer = String::new();
    encode(&mut buffer, &registry)?;

    Ok(buffer)
}

// Gets the node's name
fn get_node_name(node: &Node) -> Result<String> {
    node.status
        .as_ref() // gets a reference from &Option<T> -> Option<&T> without taking ownership
        .ok_or(anyhow!("failed to get status"))
        .and_then(|status| {
            status
                .conditions
                .as_ref()
                .ok_or(anyhow!("failed to get conditions"))
        })
        .and_then(|conditions| {
            if conditions
                .iter()
                .any(|c| c.status == "True" && c.type_ == "Ready")
            {
                Ok(node.metadata.name.clone().unwrap())
            } else {
                Err(anyhow!("failed to get node name"))
            }
        })
}

async fn get_node_stats(node_name: String) -> Result<NodeStats> {
    let auth_header = if Path::new(SERVICE_ACCOUNT_TOKEN).exists() {
        format!("Bearer {}", fs::read_to_string(SERVICE_ACCOUNT_TOKEN)?)
    } else {
        "".to_string()
    };
    let mut api_url = Url::parse(
        env::var("API_HOST")
            .unwrap_or_else(|_| "https://kubernetes.default.svc".to_string())
            .as_str(),
    )?;
    api_url.set_path(format!("/api/v1/nodes/{}/proxy/stats/summary", node_name,).as_str());

    // if there is a certificate, use it on the client connection
    let client = if Path::new(CLIENT_CERT_BUNDLE).exists() {
        let mut buf = Vec::new();
        fs::File::open(CLIENT_CERT_BUNDLE)?.read_to_end(&mut buf)?;
        let cert = reqwest::Certificate::from_pem(&buf)?;
        reqwest::Client::builder()
            .add_root_certificate(cert)
            .build()?
    } else {
        reqwest::Client::new()
    };

    let response = client
        .get(api_url)
        .header(AUTHORIZATION, &auth_header)
        .send()
        .await?;

    info!("fetching stats from {}", node_name);

    if !response.status().is_success() {
        error!("request fail with status {}", response.status());
    };

    let body = response.text().await?;
    let stats: NodeStats = serde_json::from_str(&body)?;

    Ok(stats)
}

async fn handler(req: Request<body::Incoming>) -> Result<Response<String>, hyper::http::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/metrics") => match get_stats().await {
            Ok(body) => Response::builder()
                .status(StatusCode::OK)
                .header(
                    header::CONTENT_TYPE,
                    "application/openmetrics-text; version=1.0.0; charset=utf-8",
                )
                .body(body),
            Err(err) => {
                error!("failed to reach remote server: [{}]", err.to_string());
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body("Bad gateway".to_string())
            }
        },
        (&Method::GET, "/health") => Response::builder()
            .status(StatusCode::OK)
            .body("ok".to_string()),
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not found".to_string()),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", option_env!("LOG_LEVEL").unwrap_or("info"));
    env_logger::init();

    info!(
        "Starting kube-stats-exporter version={} revision={}",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH")
    );

    let addr = "0.0.0.0:9100";
    info!("Listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(handler))
                .await
            {
                eprintln!("Server error: {}", err);
            }
        });
    }
}
