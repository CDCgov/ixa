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
