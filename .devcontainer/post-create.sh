cd /workspaces/ixa

# Install everything listed under the [tools] section of mise.toml
mise trust
mise install

# See https://mise.jdx.dev/cli/activate.html
# This makes sure that tools installed with mise (e.g., hyperfine, mdbook)
# are available in your PATH
echo 'eval "$(/usr/local/bin/mise activate bash)"' >> ~/.bashrc
source ~/.bashrc

# Add pre-commit hooks
mise install:hooks

# Copy the custom welcome message
# See https://github.com/orgs/community/discussions/43534
sudo cp .devcontainer/terminal-welcome.txt /usr/local/etc/vscode-dev-containers/first-run-notice.txt
