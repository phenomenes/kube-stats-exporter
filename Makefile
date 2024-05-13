NAME        = kube-stats-exporter
IMAGE       = kube-stats-exporter
TAG         ?= test
K8S_VERSION = "1.29"

clean:
	cargo clean

build:
	cargo build

image: clean-image
	docker build \
		--no-cache \
		--rm \
		-t $(IMAGE):$(TAG) \
		-f Dockerfile .

push-image:
	docker build \
		--no-cache \
		--rm \
		-t ghrc.io/phenomenes/$(IMAGE):$(TAG) .

clean-image:
	-docker rmi -f $(IMAGE)/$(TAG) >/dev/null 2>&1

chart:	lint-chart
	helm package charts

lint-chart:
	helm lint charts/ --values charts/values.yaml

minikube:
	minikube delete || true
	minikube start \
		--kubernetes-version="$(K8S_VERSION)" \
	        --cpus=2 \
		--memory=4800MB
