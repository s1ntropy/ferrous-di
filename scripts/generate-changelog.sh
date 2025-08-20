#!/bin/bash

# Generate changelog for ferrous-di using git-cliff
# This script can be run manually or as part of CI/CD

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CHANGELOG_FILE="CHANGELOG.md"
CLIFF_CONFIG=".cliff.toml"

# Functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if git-cliff is installed
check_dependencies() {
    if ! command -v git-cliff &> /dev/null; then
        log_error "git-cliff is not installed. Please install it first:"
        echo "  cargo install git-cliff"
        exit 1
    fi
    
    if ! git rev-parse --git-dir > /dev/null 2>&1; then
        log_error "Not in a git repository"
        exit 1
    fi
}

# Validate configuration
validate_config() {
    if [[ ! -f "$CLIFF_CONFIG" ]]; then
        log_error "Configuration file $CLIFF_CONFIG not found"
        exit 1
    fi
    
    log_info "Using configuration: $CLIFF_CONFIG"
}

# Generate changelog for specific version or unreleased changes
generate_changelog() {
    local version="$1"
    local output_file="$2"
    local from_tag="$3"
    local to_tag="$4"
    
    log_info "Generating changelog..."
    
    # Build git-cliff command
    local cmd="git-cliff --config $CLIFF_CONFIG"
    
    if [[ -n "$version" ]]; then
        cmd="$cmd --tag $version"
    fi
    
    if [[ -n "$from_tag" ]]; then
        if [[ -n "$to_tag" ]]; then
            cmd="$cmd $from_tag..$to_tag"
        else
            cmd="$cmd $from_tag.."
        fi
    fi
    
    if [[ -n "$output_file" ]]; then
        cmd="$cmd --output $output_file"
    fi
    
    log_info "Running: $cmd"
    
    if eval "$cmd"; then
        log_success "Changelog generated successfully"
        if [[ -n "$output_file" ]]; then
            log_info "Output written to: $output_file"
        fi
    else
        log_error "Failed to generate changelog"
        exit 1
    fi
}

# Update existing changelog
update_changelog() {
    local version="$1"
    
    if [[ -f "$CHANGELOG_FILE" ]]; then
        log_info "Backing up existing changelog"
        cp "$CHANGELOG_FILE" "${CHANGELOG_FILE}.backup"
    fi
    
    generate_changelog "$version" "$CHANGELOG_FILE" "" ""
}

# Preview unreleased changes
preview_unreleased() {
    log_info "Previewing unreleased changes"
    generate_changelog "" "" "$(git describe --tags --abbrev=0 2>/dev/null || echo '')" "HEAD"
}

# Show recent commits for debugging
show_recent_commits() {
    log_info "Recent commits:"
    git log --oneline --no-merges -10 --pretty=format:"%C(yellow)%h%C(reset) %C(blue)%an%C(reset) %s"
}

# Validate conventional commits
validate_commits() {
    log_info "Validating recent commits follow conventional commit format..."
    
    local invalid_commits=()
    local commit_pattern="^(feat|fix|docs|style|refactor|test|chore|perf|ci|build)(\(.+\))?(!)?: .+"
    
    # Check last 10 commits
    while IFS= read -r commit; do
        if ! [[ "$commit" =~ $commit_pattern ]]; then
            invalid_commits+=("$commit")
        fi
    done < <(git log --oneline --no-merges -10 --pretty=format:"%s")
    
    if [[ ${#invalid_commits[@]} -gt 0 ]]; then
        log_warning "Found commits that don't follow conventional commit format:"
        for commit in "${invalid_commits[@]}"; do
            echo "  - $commit"
        done
        echo ""
        echo "Conventional commit format: type(scope): description"
        echo "Types: feat, fix, docs, style, refactor, test, chore, perf, ci, build"
        echo "Example: feat(resolver): add async resolution support"
    else
        log_success "All recent commits follow conventional commit format"
    fi
}

# Show help
show_help() {
    cat << EOF
Generate changelog for ferrous-di using conventional commits

USAGE:
    $0 [COMMAND] [OPTIONS]

COMMANDS:
    update [VERSION]    Update CHANGELOG.md with new version
    preview            Preview unreleased changes
    validate           Validate recent commits follow conventional format
    show-commits       Show recent commits
    help               Show this help

OPTIONS:
    VERSION            Version tag (e.g., v1.2.3). If not provided, generates unreleased section

EXAMPLES:
    $0 update v1.2.3           # Update changelog for version 1.2.3
    $0 update                  # Update changelog with unreleased changes
    $0 preview                 # Preview what would be in next release
    $0 validate                # Check if commits follow conventional format

REQUIREMENTS:
    - git-cliff must be installed: cargo install git-cliff
    - Repository must use conventional commits
    - .cliff.toml configuration file must exist

EOF
}

# Main execution
main() {
    local command="${1:-help}"
    
    case "$command" in
        "update")
            check_dependencies
            validate_config
            local version="${2:-}"
            update_changelog "$version"
            ;;
        "preview")
            check_dependencies
            validate_config
            preview_unreleased
            ;;
        "validate")
            validate_commits
            ;;
        "show-commits")
            show_recent_commits
            ;;
        "help"|"-h"|"--help")
            show_help
            ;;
        *)
            log_error "Unknown command: $command"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"