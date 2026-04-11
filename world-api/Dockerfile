FROM rust:1.94-slim-bookworm

# System deps for sqlx/postgres + TLS
RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    ca-certificates \
    pkg-config \
    libssl-dev \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# (Optional) cache deps layer
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build

EXPOSE 3001

CMD ["cargo", "run"]
