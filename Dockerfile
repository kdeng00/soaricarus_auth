FROM rust:1.97 as builder

# Set the working directory inside the container
WORKDIR /usr/src/app

# Install build dependencies if needed (e.g., git for cloning)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl3 \
    ca-certificates \
    openssh-client git \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p -m 0700 ~/.ssh && \
    ssh-keyscan git.kundeng.us >> ~/.ssh/known_hosts

# Copy Cargo manifests
COPY Cargo.toml Cargo.lock ./

RUN --mount=type=ssh mkdir src && \
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    cargo build --release --quiet && \
    rm -rf src target/release/deps/soaricarus_auth*

COPY src ./src
# If you have other directories like `templates` or `static`, copy them too
COPY .env ./.env
COPY migrations ./migrations

RUN --mount=type=ssh \
    cargo build --release --quiet

FROM debian:trixie-slim

# Install runtime dependencies if needed (e.g., SSL certificates)
RUN apt-get update && apt-get install -y ca-certificates libssl-dev libssl3 && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/local/bin

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/soaricarus_auth .

# Copy other necessary files like .env (if used for runtime config) or static assets
# It's generally better to configure via environment variables in Docker though
COPY --from=builder /usr/src/app/.env .
COPY --from=builder /usr/src/app/migrations ./migrations

EXPOSE 8001

# Set the command to run your application
# Ensure this matches the binary name copied above
CMD ["./soaricarus_auth"]
