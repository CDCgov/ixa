#!/bin/sh
#
# Run in a new project directory.
#
# Install the default blank template:
# curl -s -f -L https://raw.githubusercontent.com/CDCgov/ixa/main/scripts/setup_new_ixa_project.sh | sh
#
# Install the parameters template:
# curl -s -f -L https://raw.githubusercontent.com/CDCgov/ixa/main/scripts/setup_new_ixa_project.sh | sh -s -- --template parameters
#
# Use a specific Ixa branch instead of the released crate:
# curl -s -f -L https://raw.githubusercontent.com/CDCgov/ixa/main/scripts/setup_new_ixa_project.sh | sh -s -- <ixa-branch> --template <template-name>

ixa_branch="main"
ixa_branch_was_set="false"
template="blank"

usage() {
    printf '%s\n' \
        "Usage: setup_new_ixa_project.sh [ixa-branch] [--template <template-name>]" \
        "" \
        "Set up a new Ixa project in the current directory." \
        "" \
        "Arguments:" \
        "  ixa-branch              Optional Ixa Git branch. Uses the released crate when omitted." \
        "" \
        "Options:" \
        "  --template <name>       Project template: blank (default) or parameters." \
        "  -h, --help              Show this help message."
}

fail() {
    printf 'Error: %s\n' "$1" >&2
    printf "Run 'setup_new_ixa_project.sh --help' for usage.\n" >&2
    exit 2
}

urlencode() {
    urlencode_tmp=$1
    urlencode_encoded=""

    while [ -n "$urlencode_tmp" ]; do
        urlencode_rest="${urlencode_tmp#?}"
        urlencode_first="${urlencode_tmp%"$urlencode_rest"}"
        case "$urlencode_first" in
            [a-zA-Z0-9.~_-])
                urlencode_encoded="$urlencode_encoded$urlencode_first"
                ;;
            *)
                urlencode_encoded="$urlencode_encoded$(printf '%%%02X' "'$urlencode_first")"
                ;;
        esac
        urlencode_tmp=$urlencode_rest
    done
    printf '%s\n' "$urlencode_encoded"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --template)
            if [ "$#" -lt 2 ] || [ -z "$2" ]; then
                fail "Missing value for --template."
            fi
            template=$2
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        --)
            shift
            ;;
        -*)
            fail "Unknown option: $1"
            ;;
        *)
            if [ "$ixa_branch_was_set" = "true" ]; then
                fail "Only one Ixa branch may be specified."
            fi
            ixa_branch=$1
            ixa_branch_was_set="true"
            shift
            ;;
    esac
done

case "$template" in
    blank | parameters) ;;
    *)
        fail "Unknown template '$template'. Expected 'blank' or 'parameters'."
        ;;
esac

ixa_asset_ref=$(urlencode "$ixa_branch")
# CI overrides the asset base so pull requests test files from their checkout.
if [ -n "${IXA_SETUP_ASSET_BASE_URL:-}" ]; then
    ixa_asset_base_url=${IXA_SETUP_ASSET_BASE_URL%/}
else
    ixa_asset_base_url="https://raw.githubusercontent.com/CDCgov/ixa/$ixa_asset_ref"
fi

download_asset() {
    download_source=$1
    download_destination=$2
    download_description=$3

    if ! curl -s -f -L -o "$download_destination" \
        "$ixa_asset_base_url/$download_source"; then
        printf 'Failed to download %s from Ixa.\n' "$download_description" >&2
        exit 1
    fi
}

printf 'Setting up new Ixa project with branch %s and template %s\n' \
    "$ixa_branch" "$template"

if [ -z "$(command -v cargo)" ]; then
    printf '%s\n' \
        "cargo could not be found, run:" \
        "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
    exit 1
fi

if [ ! -f "Cargo.toml" ]; then
    printf 'Creating Cargo project\n'
    if ! cargo init; then
        printf 'Failed to create Cargo project.\n' >&2
        exit 1
    fi
fi

if [ "$ixa_branch_was_set" = "false" ]; then
    if ! cargo add ixa; then
        printf 'Failed to add the released Ixa crate.\n' >&2
        exit 1
    fi
else
    if ! cargo add --git "https://github.com/CDCgov/ixa" \
        --branch "$ixa_branch" ixa; then
        printf 'Failed to add Ixa from branch %s.\n' "$ixa_branch" >&2
        exit 1
    fi
fi

download_asset ".gitignore" ".gitignore" ".gitignore"
download_asset "clippy.toml" "clippy.toml" "clippy.toml"

case "$template" in
    blank)
        download_asset "examples/basic/main.rs" "src/main.rs" \
            "the blank template"
        ;;
    parameters)
        if ! cargo add serde --features derive; then
            printf 'Failed to add Serde for the parameters template.\n' >&2
            exit 1
        fi
        download_asset "scripts/templates/parameters/main.rs" "src/main.rs" \
            "the parameters template main.rs"
        download_asset "scripts/templates/parameters/parameters.rs" \
            "src/parameters.rs" "the parameters template parameters.rs"
        ;;
esac

printf 'Project setup complete from branch %s with template %s\n' \
    "$ixa_branch" "$template"
printf "%s\n" \
    "Run 'cargo run' to test the example code" \
    "Check out the Ixa documentation for more examples and usage: https://ixa.rs/"
