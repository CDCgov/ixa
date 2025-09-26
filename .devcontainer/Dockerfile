# syntax=docker/dockerfile:1
FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

# Prefer stricter shells during RUN
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]

# Load args
ARG USERNAME=vscode
ARG CODESPACES_ENVIRONMENT=github
ARG FOLDER_BASENAME
ARG RUST_VERSION

RUN mkdir -p /workspaces/${FOLDER_BASENAME} && \
  chown ${USERNAME}:${USERNAME} /workspaces/${FOLDER_BASENAME}

# Ensure .bashrc and .zshrc exist
RUN touch /home/${USERNAME}/.bashrc /home/${USERNAME}/.zshrc

# Base dependencies
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y \
    lldb curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

USER ${USERNAME}

# Show dirty state in prompt
RUN git config --global devcontainers-theme.show-dirty 1

# Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --default-toolchain "${RUST_VERSION}"
ENV PATH="/home/$USERNAME/.cargo/bin:${PATH}"
RUN rustup --version && cargo --version
RUN rustup component add rustfmt clippy && \
  rustup target add wasm32-unknown-unknown

# Mise
RUN curl -fsSL https://mise.run | sh
ENV PATH="/home/${USERNAME}/.local/bin:$PATH"
RUN mise --version

# Install dependencies with mise
COPY mise.toml /workspaces/${FOLDER_BASENAME}/mise.toml
WORKDIR /workspaces/${FOLDER_BASENAME}
RUN mise trust && \
  mise install

# Shell setup
RUN \
  for shell in bash zsh; do \
    # Rust
    echo '. ~/.cargo/env' >> /home/${USERNAME}/.${shell}rc && \
    # Mise
    echo "eval \"\$(mise activate ${shell})\"" >> /home/${USERNAME}/.${shell}rc; \
  done
