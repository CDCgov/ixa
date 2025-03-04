# to be run in a new project directory
# Usage: ./setup_new_ixa_project.sh <ixa-branch>
# ixa-branch: the branch of ixa to use, default is release
ixa_branch="release"
if [ -n "$1" ]; then
    ixa_branch=$1
fi

# function to check if last shell command was successful, if not print input message and exit
check_success() {
    if [ $? -ne 0 ]; then
        echo $1
        exit
    fi
}

echo "Setting up new ixa project with branch $ixa_branch"

# check if cargo is installed
if ! command -v cargo &> /dev/null
then
    echo "cargo could not be found, run:"
    echo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit
fi

# check if cargo.toml does not exists
if [ ! -f "Cargo.toml" ]; then
    echo "Creating Cargo project"
    cargo init
fi

cargo add --git "https://github.com/CDCgov/ixa" ixa --branch $ixa_branch

# add .gitignore from ixa
curl -s -f -o .gitignore --variable %B=$ixa_branch --expand-url "https://raw.githubusercontent.com/CDCgov/ixa/{{B:url}}/.gitignore"
check_success "Failed to download .gitignore from ixa"

# add the pre-commit hook from ixa
curl -s -f -o .pre-commit-config.yaml --variable %B=$ixa_branch --expand-url https://raw.githubusercontent.com/CDCgov/ixa/{{B:url}}/.pre-commit-config.yaml
check_success "Failed to download pre-commit-config.yaml from ixa"

# add github action from ixa
mkdir -p .github/workflows
curl -s -f -o .github/workflows/build-test.yaml --variable %B=$ixa_branch --expand-url https://raw.githubusercontent.com/CDCgov/ixa/{{B:url}}/scripts/template/.github/workflows/build-test.yaml
check_success "Failed to download build-test.yaml from ixa"
curl -s -f -o .github/workflows/pre-commit.yaml --variable %B=$ixa_branch --expand-url https://raw.githubusercontent.com/CDCgov/ixa/{{B:url}}/scripts/template/.github/workflows/pre-commit.yaml
check_success "Failed to download pre-commit.yaml from ixa"

# override main.rs with ixa basic example
curl -s -f -o src/main.rs --variable %B=$ixa_branch --expand-url https://raw.githubusercontent.com/CDCgov/ixa/{{B:url}}/examples/basic/main.rs
check_success "Failed to download main.rs from ixa"

echo "Project setup complete from branch $ixa_branch"
