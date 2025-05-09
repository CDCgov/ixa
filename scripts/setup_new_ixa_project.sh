# To be run in a new project directory.
# curl -s -f -L https://raw.githubusercontent.com/CDCgov/ixa/main/scripts/setup_new_ixa_project.sh | sh
# or if you want to use a specific branch and not the cargo release
# curl -s -f -L https://raw.githubusercontent.com/CDCgov/ixa/main/scripts/setup_new_ixa_project.sh | sh -s <ixa-branch>
# ixa-branch: the branch of ixa to use, default is main
ixa_branch="main"

urlencode() {
    local tmp="${1}"
    local encoded=""

    while [ -n "$tmp" ]; do
        rest="${tmp#?}"    # All but the first character of the string
        first="${tmp%"$rest"}"    # Remove $rest, and you're left with the first character
        case "$first" in
            [a-zA-Z0-9.~_-]) encoded="$encoded$first" ;;
            *) encoded="$encoded$(printf '%%%02X' "'$first")" ;;
        esac
        tmp="$rest"
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

if [ -z "$1" ]; then
    cargo add ixa
else
    cargo add --git "https://github.com/CDCgov/ixa" ixa --branch $ixa_branch
fi

# add .gitignore from ixa
curl -s -f -o .gitignore "https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/.gitignore"
check_success "Failed to download .gitignore from ixa"

# add the pre-commit hook from ixa
curl -s -f -o .pre-commit-config.yaml https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/.pre-commit-config.yaml
check_success "Failed to download pre-commit-config.yaml from ixa"

# add the clippy.toml from ixa
curl -s -f -o .clippy.toml https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/.clippy.toml
check_success "Failed to download clippy.toml from ixa"

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
