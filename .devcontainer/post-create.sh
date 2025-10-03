cd /workspaces/ixa

# Copy the custom welcome message
# See https://github.com/orgs/community/discussions/43534
sudo cp .devcontainer/terminal-welcome.txt /usr/local/etc/vscode-dev-containers/first-run-notice.txt

sudo chown vscode:rustlang "${CARGO_HOME}"
sudo chmod -R 775 "${CARGO_HOME}"
sudo chown vscode:rustlang "${RUSTUP_HOME}"
sudo chmod -R 775 "${RUSTUP_HOME}"

# See https://mise.jdx.dev/cli/activate.html
# This makes sure that tools installed with mise (e.g., hyperfine, mdbook)
# are available in your PATH
echo 'eval "$(/usr/local/bin/mise activate bash)"' >> ~/.bashrc
source ~/.bashrc

# Install everything listed under the [tools] section of mise.toml
mise trust

# Add pre-commit hooks
mise install:hooks

