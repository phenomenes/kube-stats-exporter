# kube-stats-exporter

kube-stats-exporter is a Prometheus exporter that collects ephemeral storage
metrics for Kubernetes pods by accessing the Kubelet’s Summary API. These
metrics are not available on the Kubelet’s /metrics endpoint, as noted in
kubernetes/kubernetes#69507.

## Features

* Collects pod-level ephemeral storage metrics, including available, capacity,
and used bytes.
* Exposes metrics in Prometheus format for easy integration.

## Prerequisites

* Rust toolchain for building the exporter.
* Access to a Kubernetes cluster with appropriate permissions.

## Installation

Clone the repository:

```shell
git clone https://github.com/phenomenes/kube-stats-exporter.git
cd kube-stats-exporter
```

Build the binary:

```shell
cargo build --release
```

Run the exporter:

```shell
kubectl proxy --port=8001
export API_HOST=127.0.0.1:8001
./target/release/kube-stats-exporter
```

## Metrics

The exporter provides the following metrics:

* `kube_pod_ephemeral_storage_available_bytes`: Available ephemeral storage bytes per pod.
* `kube_pod_ephemeral_storage_capacity_bytes`: Total ephemeral storage capacity bytes per pod.
* `kube_pod_ephemeral_storage_used_bytes`: Used ephemeral storage bytes per pod.

## Usage

After starting the exporter, metrics can be accessed at `http://localhost:9100/metrics`.
Integrate this endpoint with your Prometheus server to begin scraping the metrics.

## License

This project is licensed under the MIT License.
