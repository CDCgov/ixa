# to be run in a new project directory
# Usage: ./setup_new_ixa_project.sh <ixa-branch>
# or directly from github:
# curl -s -f -L https://raw.githubusercontent.com/CDCgov/ixa/master/scripts/setup_new_ixa_project.sh | bash -s <ixa-branch>
# ixa-branch: the branch of ixa to use, default is release
ixa_branch="release"

urlencode() {
    local string="${1}"
    local length="${#string}"
    local encoded=""

    for (( i = 0; i < length; i++ )); do
        char="${string:i:1}"
        case "$char" in
            [a-zA-Z0-9.~_-]) encoded+="$char" ;;
            *) encoded+=$(printf '%%%02X' "'$char") ;;
        esac
    done

    echo "$encoded"
}

if [ -n "$1" ]; then
    ixa_branch=$(urlencode $1)
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
if [ -z "$(command -v cargo)" ]; then
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
curl -s -f -o .gitignore "https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/.gitignore"
check_success "Failed to download .gitignore from ixa"

# add the pre-commit hook from ixa
curl -s -f -o .pre-commit-config.yaml https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/.pre-commit-config.yaml
check_success "Failed to download pre-commit-config.yaml from ixa"

# add github action from ixa
mkdir -p .github/workflows
curl -s -f -o .github/workflows/build-test.yaml https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/scripts/template/.github/workflows/build-test.yaml
check_success "Failed to download build-test.yaml from ixa"
curl -s -f -o .github/workflows/pre-commit.yaml https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/scripts/template/.github/workflows/pre-commit.yaml
check_success "Failed to download pre-commit.yaml from ixa"

# override main.rs with ixa basic example
curl -s -f -o src/main.rs https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/examples/basic/main.rs
check_success "Failed to download main.rs from ixa"

echo "Project setup complete from branch $ixa_branch"
