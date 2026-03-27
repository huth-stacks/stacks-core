FROM rust:bookworm AS build

ARG STACKS_NODE_VERSION="No Version Info"
ARG GIT_BRANCH='No Branch Info'
ARG GIT_COMMIT='No Commit Info'

WORKDIR /src
COPY . .
RUN mkdir /out
RUN rustup toolchain install stable
RUN cargo build --features monitoring_prom,slog_json --release
RUN cp -R target/release/. /out

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends curl && rm -rf /var/lib/apt/lists/*
COPY --from=build /out/stacks-node /out/stacks-signer /out/stacks-inspect /bin/
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -sf http://localhost:20443/v2/info > /dev/null || exit 1
CMD ["stacks-node", "mainnet"]
