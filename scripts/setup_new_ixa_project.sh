# to be run in a new project directory
# Usage: ./setup_new_ixa_project.sh <ixa-branch>
# ixa-branch: the branch of ixa to use, default is release
ixa_branch="release"
if [ -n "$1" ]; then
    ixa_branch=$1
fi

# check if cargo is installed
if ! command -v cargo &> /dev/null
then
    echo "cargo could not be found, run:"
    echo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit
fi

# check if cargo.toml does not exists
if ! [ -f "Cargo.toml" ]; then
    echo "Creating Cargo project"
    cargo init
fi

cargo add --git "https://github.com/CDCgov/ixa" ixa --branch $ixa_branch

# add .gitignore from ixa
curl -o .gitignore https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/.gitignore

# add the pre-commit hook from ixa
curl -o .pre-commit-config.yaml https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/.pre-commit-config.yaml

# add github action from ixa
mkdir -p .github/workflows
curl -o .github/workflows/build-test.yaml https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/template/.github/workflows/build-test.yaml
curl -o .github/workflows/pre-commit.yaml https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/template/.github/workflows/pre-commit.yaml

# override main.rs with ixa basic example
curl -o src/main.rs https://raw.githubusercontent.com/CDCgov/ixa/$ixa_branch/examples/basic/main.rs

echo "Project setup complete"
