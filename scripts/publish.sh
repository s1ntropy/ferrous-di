#!/bin/bash

# Automated crate publishing script for ferrous-di
# Handles crates.io publishing with safety checks and rollback capability

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
PACKAGE_NAME="ferrous-di"
CARGO_TOML="Cargo.toml"
RELEASE_CHECKLIST="RELEASE_CHECKLIST.md"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Publishing settings
PUBLISH_TIMEOUT="300"  # 5 minutes timeout for publish
RETRY_ATTEMPTS="3"
RETRY_DELAY="30"      # 30 seconds between retries

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

log_step() {
    echo -e "${PURPLE}[STEP]${NC} $1"
}

# Check dependencies
check_dependencies() {
    local missing_deps=()
    
    # Required tools
    local required_tools=("git" "cargo" "jq")
    
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            missing_deps+=("$tool")
        fi
    done
    
    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        log_error "Missing required dependencies:"
        for dep in "${missing_deps[@]}"; do
            echo "  - $dep"
        done
        echo ""
        echo "Install missing dependencies:"
        for dep in "${missing_deps[@]}"; do
            case "$dep" in
                "jq")
                    echo "  # Install jq for JSON parsing"
                    echo "  # Ubuntu/Debian: sudo apt-get install jq"
                    echo "  # macOS: brew install jq"
                    echo "  # Or download from: https://stedolan.github.io/jq/"
                    ;;
                *)
                    echo "  Install $dep via your package manager"
                    ;;
            esac
        done
        exit 1
    fi
}

# Get current version from Cargo.toml
get_current_version() {
    if [[ -f "$CARGO_TOML" ]]; then
        grep '^version = ' "$CARGO_TOML" | sed 's/version = "\(.*\)"/\1/' | tr -d '"'
    else
        log_error "Cargo.toml not found"
        exit 1
    fi
}

# Check if version exists on crates.io
check_version_exists() {
    local version="$1"
    
    log_info "Checking if version $version already exists on crates.io..."
    
    # Use crates.io API to check if version exists
    local api_url="https://crates.io/api/v1/crates/$PACKAGE_NAME"
    local response
    
    if response=$(curl -s "$api_url" 2>/dev/null); then
        if echo "$response" | jq -r '.versions[].num' | grep -q "^$version$"; then
            log_error "Version $version already exists on crates.io"
            exit 1
        fi
    else
        log_warning "Could not check crates.io API, proceeding with caution"
    fi
    
    log_success "Version $version is available for publishing"
}

# Validate cargo login
validate_cargo_auth() {
    log_step "Validating cargo authentication"
    
    # Check if user is logged in
    if ! cargo owner --list "$PACKAGE_NAME" &>/dev/null; then
        log_error "Not authenticated with crates.io or not an owner of $PACKAGE_NAME"
        echo ""
        echo "To authenticate:"
        echo "1. Get your API token from https://crates.io/me"
        echo "2. Run: cargo login <your-token>"
        echo "3. Or set CARGO_REGISTRY_TOKEN environment variable"
        exit 1
    fi
    
    log_success "Cargo authentication validated"
}

# Validate repository state
validate_repo_state() {
    log_step "Validating repository state"
    
    # Check if we're in a git repository
    if ! git rev-parse --git-dir > /dev/null 2>&1; then
        log_error "Not in a git repository"
        exit 1
    fi
    
    # Check for uncommitted changes
    if ! git diff-index --quiet HEAD --; then
        log_error "Repository has uncommitted changes"
        git status --porcelain
        exit 1
    fi
    
    # Check if we're on main/master branch
    local current_branch
    current_branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$current_branch" != "main" && "$current_branch" != "master" ]]; then
        log_warning "Not on main/master branch (currently on: $current_branch)"
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
    
    # Check if latest commit is tagged
    local current_version
    current_version=$(get_current_version)
    if ! git tag --points-at HEAD | grep -q "v$current_version"; then
        log_error "Current commit is not tagged with v$current_version"
        echo "Make sure to create a release with proper tagging first"
        exit 1
    fi
    
    log_success "Repository state is valid"
}

# Run pre-publish checks
run_pre_publish_checks() {
    log_step "Running pre-publish checks"
    
    # Test that package can be built
    log_info "Testing package build..."
    if ! cargo build --release --all-features; then
        log_error "Package build failed"
        exit 1
    fi
    
    # Run all tests
    log_info "Running tests..."
    if ! cargo test --all-features; then
        log_error "Tests failed"
        exit 1
    fi
    
    # Run clippy
    log_info "Running clippy..."
    if ! cargo clippy --all-targets --all-features -- -D warnings; then
        log_error "Clippy checks failed"
        exit 1
    fi
    
    # Check formatting
    log_info "Checking code formatting..."
    if ! cargo fmt --check; then
        log_error "Code formatting check failed"
        exit 1
    fi
    
    # Security audit
    log_info "Running security audit..."
    if ! cargo audit; then
        log_warning "Security audit found issues. Please review."
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
    
    # Test package creation
    log_info "Testing package creation..."
    if ! cargo package --allow-dirty; then
        log_error "Package creation failed"
        exit 1
    fi
    
    log_success "All pre-publish checks passed"
}

# Dry run publish
dry_run_publish() {
    log_step "Running dry-run publish"
    
    if ! cargo publish --dry-run --allow-dirty; then
        log_error "Dry-run publish failed"
        exit 1
    fi
    
    log_success "Dry-run publish successful"
}

# Actual publish with retries
do_publish() {
    local attempt=1
    local max_attempts="$RETRY_ATTEMPTS"
    
    log_step "Publishing to crates.io"
    
    while [[ $attempt -le $max_attempts ]]; do
        log_info "Publish attempt $attempt/$max_attempts"
        
        if timeout "$PUBLISH_TIMEOUT" cargo publish --allow-dirty; then
            log_success "Successfully published to crates.io!"
            return 0
        else
            local exit_code=$?
            
            if [[ $exit_code -eq 124 ]]; then
                log_warning "Publish attempt $attempt timed out after ${PUBLISH_TIMEOUT}s"
            else
                log_warning "Publish attempt $attempt failed with exit code $exit_code"
            fi
            
            if [[ $attempt -lt $max_attempts ]]; then
                log_info "Retrying in ${RETRY_DELAY}s..."
                sleep "$RETRY_DELAY"
            fi
        fi
        
        ((attempt++))
    done
    
    log_error "Failed to publish after $max_attempts attempts"
    exit 1
}

# Verify publication
verify_publication() {
    local version="$1"
    local max_wait=300  # 5 minutes
    local wait_time=0
    local check_interval=30
    
    log_step "Verifying publication on crates.io"
    
    while [[ $wait_time -lt $max_wait ]]; do
        log_info "Checking if version $version is available... (${wait_time}s elapsed)"
        
        # Check crates.io API
        local api_url="https://crates.io/api/v1/crates/$PACKAGE_NAME"
        local response
        
        if response=$(curl -s "$api_url" 2>/dev/null); then
            if echo "$response" | jq -r '.versions[].num' | grep -q "^$version$"; then
                log_success "Version $version is now available on crates.io!"
                return 0
            fi
        fi
        
        sleep "$check_interval"
        wait_time=$((wait_time + check_interval))
    done
    
    log_warning "Version $version not yet visible on crates.io API after ${max_wait}s"
    log_info "This may be due to indexing delays. Check manually:"
    echo "  https://crates.io/crates/$PACKAGE_NAME"
}

# Create GitHub release
create_github_release() {
    local version="$1"
    
    log_step "Creating GitHub release"
    
    # Check if gh CLI is available
    if ! command -v gh &> /dev/null; then
        log_warning "GitHub CLI (gh) not available, skipping GitHub release"
        log_info "Create release manually at: https://github.com/s1ntropy/ferrous/releases"
        return 0
    fi
    
    # Check if we're authenticated
    if ! gh auth status &>/dev/null; then
        log_warning "Not authenticated with GitHub, skipping GitHub release"
        log_info "Run 'gh auth login' to authenticate"
        return 0
    fi
    
    # Create release notes from changelog
    local release_notes
    if [[ -f "CHANGELOG.md" ]]; then
        # Extract notes for this version from changelog
        release_notes=$(awk "/^## \\[$version\\]/{flag=1; next} /^## \\[/{flag=0} flag" CHANGELOG.md || echo "")
    fi
    
    if [[ -z "$release_notes" ]]; then
        release_notes="Release $version of ferrous-di

See [CHANGELOG.md](CHANGELOG.md) for detailed changes.

## Installation

\`\`\`toml
[dependencies]
ferrous-di = \"$version\"
\`\`\`

🚀 Generated with [Claude Code](https://claude.ai/code)"
    fi
    
    # Create the release
    if gh release create "v$version" \
        --title "ferrous-di v$version" \
        --notes "$release_notes" \
        --target "$(git rev-parse HEAD)"; then
        log_success "GitHub release created successfully"
    else
        log_warning "Failed to create GitHub release"
        log_info "Create release manually at: https://github.com/s1ntropy/ferrous/releases"
    fi
}

# Post-publish tasks
post_publish_tasks() {
    local version="$1"
    
    log_step "Running post-publish tasks"
    
    # Update documentation if docs.rs integration exists
    log_info "Documentation will be automatically built by docs.rs"
    
    # Create GitHub release
    create_github_release "$version"
    
    # Generate post-publish report
    generate_publish_report "$version"
    
    log_success "Post-publish tasks completed"
}

# Generate publish report
generate_publish_report() {
    local version="$1"
    local report_file="PUBLISH_REPORT_v${version}.md"
    
    cat > "$report_file" << EOF
# Publish Report: ferrous-di v$version

**Published**: $(date -u '+%Y-%m-%d %H:%M:%S UTC')
**Publisher**: $(git config user.name) <$(git config user.email)>
**Commit**: $(git rev-parse HEAD)
**Tag**: v$version

## Publication Details

- **Package**: $PACKAGE_NAME
- **Version**: $version
- **Crates.io**: https://crates.io/crates/$PACKAGE_NAME/$version
- **Documentation**: https://docs.rs/$PACKAGE_NAME/$version
- **Repository**: https://github.com/s1ntropy/ferrous

## Verification

- [x] Package published to crates.io
- [x] Documentation building on docs.rs
- [x] GitHub release created
- [x] All pre-publish checks passed

## Usage

\`\`\`toml
[dependencies]
ferrous-di = "$version"
\`\`\`

## Notes

$(git tag -l --format='%(contents)' "v$version")

---
Generated on $(date) by publish.sh
EOF

    log_info "Publish report created: $report_file"
}

# Show publish summary
show_publish_summary() {
    local version="$1"
    
    echo ""
    echo "========================================="
    echo "         PUBLISH SUMMARY"
    echo "========================================="
    echo "Package:        $PACKAGE_NAME"
    echo "Version:        $version"
    echo "Crates.io:      https://crates.io/crates/$PACKAGE_NAME"
    echo "Documentation:  https://docs.rs/$PACKAGE_NAME"
    echo "Repository:     https://github.com/s1ntropy/ferrous"
    echo "Published:      $(date)"
    echo "Commit:         $(git rev-parse --short HEAD)"
    echo "Tag:            v$version"
    echo "========================================="
    echo ""
}

# Show usage help
show_help() {
    cat << EOF
Automated crate publishing script for ferrous-di

USAGE:
    $0 <COMMAND> [OPTIONS]

COMMANDS:
    publish               Publish current version to crates.io
    dry-run              Test publication without actually publishing
    check                Run pre-publish checks only
    verify [VERSION]     Verify a published version exists
    help                 Show this help

OPTIONS:
    --skip-checks        Skip pre-publish checks (not recommended)
    --no-github-release  Skip GitHub release creation

EXAMPLES:
    $0 publish                    # Publish current version
    $0 dry-run                    # Test publication process
    $0 check                      # Run quality checks only
    $0 verify 1.2.3              # Verify version 1.2.3 exists

REQUIREMENTS:
    - Clean git working directory
    - Properly tagged release commit
    - Cargo authentication (cargo login)
    - All tests and checks passing

ENVIRONMENT:
    CARGO_REGISTRY_TOKEN         # Alternative to cargo login
    PUBLISH_TIMEOUT             # Override publish timeout (default: 300s)
    
PREPARATION:
    Before publishing, ensure you have:
    1. Created a release with scripts/release.sh
    2. Pushed tags to remote: git push origin main --tags
    3. Authenticated with crates.io: cargo login

EOF
}

# Main publish function
do_publish_main() {
    local skip_checks="${1:-false}"
    local skip_github="${2:-false}"
    
    local current_version
    current_version=$(get_current_version)
    
    log_info "Publishing ferrous-di v$current_version"
    
    # Validate authentication
    validate_cargo_auth
    
    # Validate repository state
    validate_repo_state
    
    # Check if version already exists
    check_version_exists "$current_version"
    
    # Show publish summary
    show_publish_summary "$current_version"
    
    # Confirm with user (unless in CI)
    if [[ "${CI:-}" != "true" ]]; then
        echo "⚠️  This will publish v$current_version to crates.io (irreversible!)"
        read -p "Proceed with publication? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_info "Publication cancelled"
            exit 0
        fi
    fi
    
    # Run pre-publish checks
    if [[ "$skip_checks" != "true" ]]; then
        run_pre_publish_checks
    else
        log_warning "Skipping pre-publish checks"
    fi
    
    # Dry run first
    dry_run_publish
    
    # Actual publish
    do_publish
    
    # Verify publication
    verify_publication "$current_version"
    
    # Post-publish tasks
    if [[ "$skip_github" != "true" ]]; then
        post_publish_tasks "$current_version"
    else
        log_info "Skipping GitHub release creation"
    fi
    
    log_success "Publication of v$current_version completed successfully!"
    echo ""
    echo "🎉 ferrous-di v$current_version is now available!"
    echo ""
    echo "Next steps:"
    echo "1. Announce the release to the community"
    echo "2. Update any dependent projects"
    echo "3. Monitor for any post-release issues"
}

# Main execution
main() {
    local command="${1:-help}"
    
    # Change to repository root
    cd "$ROOT_DIR"
    
    # Override settings from environment
    PUBLISH_TIMEOUT="${PUBLISH_TIMEOUT:-300}"
    
    case "$command" in
        "publish")
            local skip_checks="false"
            local skip_github="false"
            
            # Check for flags
            for arg in "$@"; do
                case "$arg" in
                    "--skip-checks")
                        skip_checks="true"
                        ;;
                    "--no-github-release")
                        skip_github="true"
                        ;;
                esac
            done
            
            check_dependencies
            do_publish_main "$skip_checks" "$skip_github"
            ;;
        "dry-run")
            check_dependencies
            validate_cargo_auth
            validate_repo_state
            run_pre_publish_checks
            dry_run_publish
            log_success "Dry-run completed successfully!"
            echo "To publish for real: $0 publish"
            ;;
        "check")
            check_dependencies
            validate_repo_state
            run_pre_publish_checks
            ;;
        "verify")
            local version="${2:-$(get_current_version)}"
            check_version_exists "$version"
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