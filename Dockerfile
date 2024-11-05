FROM rust:slim-bookworm AS builder
WORKDIR /spootifer
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN apt update
RUN apt -y upgrade
RUN apt install -y openssl libssl-dev pkg-config
RUN cargo build --release
EXPOSE 8080
EXPOSE 8081
# Copy binaries from the previous build stages.

FROM debian:bookworm-slim
COPY --from=flyio/litefs:0.5 /usr/local/bin/litefs /usr/local/bin/litefs
COPY --from=builder /spootifer/target/release/spootifer-rust ./spootifer/spootifer
RUN apt update
RUN apt -y upgrade
RUN apt install -y bash fuse3 curl openssl libssl-dev ca-certificates
EXPOSE 8080
EXPOSE 8081

# Copy our LiteFS configuration.
ADD litefs.app.yml litefs.app.yml
ENTRYPOINT ["litefs", "mount", "-config"]