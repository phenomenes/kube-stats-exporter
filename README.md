# kube-stats-exporter

kube-stats-exporter collects pod's ephemeral storage metrics from Kubernetes
Summary data.

```bash
kubectl get --raw /api/v1/nodes/<node_name>/proxy/stats/summary
```

These metrics are currently not available on the kubelet `/metrics` endpoint
see https://github.com/kubernetes/kubernetes/issues/69507 for more information

## Run the exporter

To run `kube-stats-metrics` locally, compile and the binary and start a proxy
to the Kubernetes API and compile/run the binary

```shell
cargo build --release
# copy the binary to your preferred location

kubectl proxy --port=8001
export API_HOST=127.0.0.1:8001 && /path/to/the/binary/kube-stats-metrics
```

Get the metrics

```shell
curl http://0.0.0.0:9100/metrics

# HELP kube_pod_ephemeral_storage_available_bytes Pod ephemeral storage available bytes.
# TYPE kube_pod_ephemeral_storage_available_bytes gauge
kube_pod_ephemeral_storage_available_bytes{pod="cilium-dxcgl",namespace="kube-system",node="ip-10-101-93-232.eu-west-2.compute.internal",uid="2b0c8b43-d32d-4e72-9bf8-e20413e495cf"} 98145308672
# HELP kube_pod_ephemeral_storage_capacity_bytes Pod ephemeral storage capacity bytes.
# TYPE kube_pod_ephemeral_storage_capacity_bytes gauge
kube_pod_ephemeral_storage_capacity_bytes{pod="image-automation-controller-75448b455d-dnkjn",namespace="flux-system",node="ip-10-101-87-27.eu-west-2.compute.internal",uid="add6c99e-9e3b-4593-a948-d09059f4d5e6"} 107361579008
# HELP kube_pod_ephemeral_storage_used_bytes Pod ephemeral storage used bytes.
# TYPE kube_pod_ephemeral_storage_used_bytes gauge
kube_pod_ephemeral_storage_used_bytes{pod="missing-container-metrics-8j96p",namespace="missing-container-metrics",node="ip-10-101-86-90.eu-west-2.compute.internal",uid="b3ce48c4-a65f-48a7-8d56-bcb9a760f5f2"} 8192
# HELP kube_stats_exporter_build_info kube-stats-exporter build info.
# TYPE kube_stats_exporter_build_info gauge
kube_stats_exporter_build_info{version="0.1.0",revision="945ef42"} 1
EOF
```
