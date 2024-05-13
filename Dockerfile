# -*- mode: dockerfile -*-
FROM rust:1.77 as builder

WORKDIR /usr/src/kube-stats-exporter
COPY . .
RUN cargo install --path .

FROM debian:bookworm-slim

WORKDIR /kube-stats-exporter

RUN groupadd --system -g 1500 kubestats && \
		useradd --system --gid 1500 \
		-M -d /kube-stats-exporter \
		-s /sbin/nologin \
		--uid 1500 kubestats \
	&& chown -R kubestats:kubestats /kube-stats-exporter \
	&& apt-get update \
	&& apt-get install -y libssl-dev curl net-tools \
	&& rm -rf /var/lib/apt/lists/*

COPY --from=builder \
	/usr/local/cargo/bin/kube-stats-exporter \
	/usr/local/bin/kube-stats-exporter

USER 1500:1500
EXPOSE 9100
ENTRYPOINT ["/usr/local/bin/kube-stats-exporter"]
