# Dockerfile for running Rust bench tests in the ixa project
FROM rust:slim

# make sure we have the latest CA certificates, including CDC ones
RUN apt-get update && apt-get install -y curl
RUN curl https://raw.githubusercontent.com/CDCgov/ocio-certificates/refs/heads/main/data/min-cdc-bundle-ca.crt | tee /usr/local/share/ca-certificates/min-cdc-bundle-ca.crt >/dev/null
RUN update-ca-certificates

# Create a user to avoid running as root
RUN useradd -m runner
USER runner
WORKDIR /home/runner/ixa

## Local repo will be mounted at runtime, not copied

# Default command: run cargo bench
CMD ["cargo", "bench", "-p", "ixa-bench"]
