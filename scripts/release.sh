#!/bin/bash

# Automated release script for ferrous-di
# Handles version bumping, changelog generation, and release preparation

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
CHANGELOG_FILE="CHANGELOG.md"
CLIFF_CONFIG=".cliff.toml"
RELEASE_CHECKLIST="RELEASE_CHECKLIST.md"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

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
    local required_tools=("git" "cargo" "git-cliff" "cargo-edit")
    
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
                "git-cliff")
                    echo "  cargo install git-cliff"
                    ;;
                "cargo-edit")
                    echo "  cargo install cargo-edit"
                    ;;
                *)
                    echo "  Install $dep via your package manager"
                    ;;
            esac
        done
        exit 1
    fi
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
        log_error "Repository has uncommitted changes. Please commit or stash them first."
        git status --porcelain
        exit 1
    fi
    
    # Check if we're on main branch
    local current_branch
    current_branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$current_branch" != "main" ]]; then
        log_warning "Not on main branch (currently on: $current_branch)"
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
    
    # Check if we can push to remote
    if ! git ls-remote origin &> /dev/null; then
        log_error "Cannot connect to remote repository"
        exit 1
    fi
    
    log_success "Repository state is valid"
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

# Determine next version based on conventional commits
determine_next_version() {
    local current_version="$1"
    local bump_type="$2"
    
    case "$bump_type" in
        "major")
            cargo set-version --bump major --dry-run | grep "version" | sed 's/.*version = "\(.*\)".*/\1/'
            ;;
        "minor")
            cargo set-version --bump minor --dry-run | grep "version" | sed 's/.*version = "\(.*\)".*/\1/'
            ;;
        "patch")
            cargo set-version --bump patch --dry-run | grep "version" | sed 's/.*version = "\(.*\)".*/\1/'
            ;;
        *)
            echo "$bump_type"  # Assume it's a specific version
            ;;
    esac
}

# Analyze commits since last tag to suggest version bump
analyze_commits_for_version_bump() {
    log_step "Analyzing commits to suggest version bump"
    
    local last_tag
    last_tag=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
    
    if [[ -z "$last_tag" ]]; then
        log_info "No previous tags found, suggesting minor version for initial release"
        echo "minor"
        return
    fi
    
    log_info "Analyzing commits since $last_tag"
    
    # Get commits since last tag
    local commits
    commits=$(git log "$last_tag..HEAD" --pretty=format:"%s" --no-merges)
    
    if [[ -z "$commits" ]]; then
        log_info "No new commits since last tag"
        echo "none"
        return
    fi
    
    # Check for breaking changes
    if echo "$commits" | grep -qE "^(feat|fix|refactor|perf)!:"; then
        log_info "Found breaking changes (marked with !)"
        echo "major"
        return
    fi
    
    # Check for BREAKING CHANGE in commit body
    if git log "$last_tag..HEAD" --pretty=format:"%B" --no-merges | grep -qi "BREAKING CHANGE"; then
        log_info "Found BREAKING CHANGE in commit body"
        echo "major"
        return
    fi
    
    # Check for new features
    if echo "$commits" | grep -qE "^feat(\(.+\))?:"; then
        log_info "Found new features"
        echo "minor"
        return
    fi
    
    # Check for fixes, performance improvements, etc.
    if echo "$commits" | grep -qE "^(fix|perf)(\(.+\))?:"; then
        log_info "Found bug fixes or performance improvements"
        echo "patch"
        return
    fi
    
    # Only chores, docs, tests, etc.
    log_info "Only maintenance commits found"
    echo "patch"
}

# Run pre-release checks
run_pre_release_checks() {
    log_step "Running pre-release checks"
    
    # Run tests
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
        log_error "Code formatting check failed. Run 'cargo fmt' to fix."
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
    
    # Check if crate can be packaged
    log_info "Testing package creation..."
    if ! cargo package --dry-run; then
        log_error "Package creation failed"
        exit 1
    fi
    
    log_success "All pre-release checks passed"
}

# Update version in Cargo.toml
update_version() {
    local new_version="$1"
    
    log_step "Updating version to $new_version"
    
    # Update Cargo.toml
    cargo set-version "$new_version"
    
    # Update Cargo.lock
    cargo update --workspace
    
    log_success "Version updated to $new_version"
}

# Generate changelog
generate_changelog() {
    local version="$1"
    
    log_step "Generating changelog for version $version"
    
    if ! "$SCRIPT_DIR/generate-changelog.sh" update "v$version"; then
        log_error "Failed to generate changelog"
        exit 1
    fi
    
    log_success "Changelog generated"
}

# Create release commit and tag
create_release_commit() {
    local version="$1"
    
    log_step "Creating release commit and tag"
    
    # Stage changes
    git add "$CARGO_TOML" Cargo.lock "$CHANGELOG_FILE"
    
    # Create commit
    local commit_message="chore(release): prepare for $version

🚀 Generated with [Claude Code](https://claude.ai/code)

Co-Authored-By: Claude <noreply@anthropic.com>"
    
    git commit -m "$commit_message"
    
    # Create tag
    git tag -a "v$version" -m "Release version $version"
    
    log_success "Release commit and tag created"
}

# Show release summary
show_release_summary() {
    local current_version="$1"
    local new_version="$2"
    
    echo ""
    echo "=================================="
    echo "    RELEASE SUMMARY"
    echo "=================================="
    echo "Package:        $PACKAGE_NAME"
    echo "Current Version: $current_version"
    echo "New Version:     $new_version"
    echo "Branch:         $(git rev-parse --abbrev-ref HEAD)"
    echo "Commit:         $(git rev-parse --short HEAD)"
    echo "Tag:            v$new_version"
    echo "=================================="
    echo ""
}

# Dry run mode
dry_run() {
    local bump_type="$1"
    local current_version
    local new_version
    
    current_version=$(get_current_version)
    new_version=$(determine_next_version "$current_version" "$bump_type")
    
    echo ""
    echo "DRY RUN MODE - No changes will be made"
    echo "======================================"
    show_release_summary "$current_version" "$new_version"
    
    echo "Would perform the following actions:"
    echo "1. Update version in Cargo.toml: $current_version → $new_version"
    echo "2. Generate changelog for v$new_version"
    echo "3. Create release commit"
    echo "4. Create git tag: v$new_version"
    echo ""
    echo "To execute the release:"
    echo "  $0 release $bump_type"
}

# Main release function
do_release() {
    local bump_type="$1"
    local skip_checks="${2:-false}"
    
    local current_version
    local new_version
    
    current_version=$(get_current_version)
    
    if [[ "$bump_type" == "auto" ]]; then
        bump_type=$(analyze_commits_for_version_bump)
        if [[ "$bump_type" == "none" ]]; then
            log_info "No new commits found, nothing to release"
            exit 0
        fi
        log_info "Auto-detected bump type: $bump_type"
    fi
    
    new_version=$(determine_next_version "$current_version" "$bump_type")
    
    # Validate repository state
    validate_repo_state
    
    # Show release summary
    show_release_summary "$current_version" "$new_version"
    
    # Confirm with user (unless in CI)
    if [[ "${CI:-}" != "true" ]]; then
        read -p "Proceed with release? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_info "Release cancelled"
            exit 0
        fi
    fi
    
    # Run pre-release checks
    if [[ "$skip_checks" != "true" ]]; then
        run_pre_release_checks
    else
        log_warning "Skipping pre-release checks"
    fi
    
    # Update version
    update_version "$new_version"
    
    # Generate changelog
    generate_changelog "$new_version"
    
    # Create release commit and tag
    create_release_commit "$new_version"
    
    log_success "Release v$new_version prepared successfully!"
    echo ""
    echo "Next steps:"
    echo "1. Review the changes: git show"
    echo "2. Push to remote: git push origin main --tags"
    echo "3. Create GitHub release"
    echo "4. Publish to crates.io: cargo publish"
    
    # Offer to push automatically
    if [[ "${CI:-}" != "true" ]]; then
        echo ""
        read -p "Push to remote now? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            git push origin "$(git rev-parse --abbrev-ref HEAD)" --tags
            log_success "Pushed to remote repository"
        fi
    fi
}

# Show usage help
show_help() {
    cat << EOF
Automated release script for ferrous-di

USAGE:
    $0 <COMMAND> [OPTIONS]

COMMANDS:
    release <BUMP_TYPE>    Create a new release
    dry-run <BUMP_TYPE>    Show what would be released without making changes
    check                  Run pre-release checks only
    version               Show current version
    analyze               Analyze commits to suggest version bump
    help                  Show this help

BUMP_TYPE:
    auto                  Automatically determine bump type from commits
    major                 Increment major version (X.y.z → X+1.0.0)
    minor                 Increment minor version (x.Y.z → x.Y+1.0)
    patch                 Increment patch version (x.y.Z → x.y.Z+1)
    <VERSION>             Set specific version (e.g., 1.2.3)

OPTIONS:
    --skip-checks         Skip pre-release checks (not recommended)

EXAMPLES:
    $0 release auto              # Automatically determine and release
    $0 release minor             # Release new minor version
    $0 dry-run patch             # See what a patch release would do
    $0 release 1.2.3             # Release specific version
    $0 check                     # Run quality checks only

ENVIRONMENT:
    CI=true                      # Skip interactive prompts (for CI/CD)

REQUIREMENTS:
    - Clean git working directory
    - All tests passing
    - No clippy warnings
    - Valid conventional commits (for auto detection)

EOF
}

# Main execution
main() {
    local command="${1:-help}"
    
    # Change to repository root
    cd "$ROOT_DIR"
    
    case "$command" in
        "release")
            local bump_type="${2:-auto}"
            local skip_checks="false"
            
            # Check for --skip-checks flag
            for arg in "$@"; do
                if [[ "$arg" == "--skip-checks" ]]; then
                    skip_checks="true"
                    break
                fi
            done
            
            check_dependencies
            do_release "$bump_type" "$skip_checks"
            ;;
        "dry-run")
            local bump_type="${2:-auto}"
            if [[ "$bump_type" == "auto" ]]; then
                bump_type=$(analyze_commits_for_version_bump)
            fi
            dry_run "$bump_type"
            ;;
        "check")
            check_dependencies
            validate_repo_state
            run_pre_release_checks
            ;;
        "version")
            echo "Current version: $(get_current_version)"
            ;;
        "analyze")
            local suggested_bump
            suggested_bump=$(analyze_commits_for_version_bump)
            echo "Suggested version bump: $suggested_bump"
            
            local current_version
            current_version=$(get_current_version)
            
            if [[ "$suggested_bump" != "none" ]]; then
                local new_version
                new_version=$(determine_next_version "$current_version" "$suggested_bump")
                echo "Next version would be: $new_version"
            fi
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