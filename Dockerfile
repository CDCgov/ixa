# Dockerfile for running Rust bench tests in the ixa project
FROM rust:slim

# make sure we have the latest CA certificates, including CDC ones
RUN apt-get update && apt-get install -y curl
RUN curl https://raw.githubusercontent.com/CDCgov/ocio-certificates/refs/heads/main/data/min-cdc-bundle-ca.crt | tee /usr/local/share/ca-certificates/min-cdc-bundle-ca.crt >/dev/null
RUN update-ca-certificates

# install missing dependencies for building some crates
RUN apt-get update &&  apt install libssl-dev pkg-config unzip -y


# Create a user to avoid running as root
RUN useradd -m runner

USER runner
WORKDIR /home/runner/ixa

# Copy ixa_setup.sh script to /home/runner and make it executable
COPY scripts/ixa_setup.sh /home/runner/ixa_setup.sh
#RUN chmod +x /home/runner/ixa_setup.sh

# Install mise for task running
RUN curl -fsSL https://mise.run | sh
ENV PATH="/home/runner/.local/bin:$PATH"
RUN mise --version

## Local repo will be mounted at runtime, not copied

# Default command: run cargo bench
CMD ["cargo", "bench", "-p", "ixa-bench"]
