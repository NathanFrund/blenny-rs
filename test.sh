#!/bin/bash
# Test runner script for Blenny
# Provides convenient commands for running different types of tests

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Check if we're in the right directory
if [ ! -f "blenny/Cargo.toml" ]; then
    print_error "Please run this script from the project root directory"
    exit 1
fi

cd blenny

case "${1:-all}" in
    "unit")
        print_info "Running unit tests..."
        cargo test --lib
        print_success "Unit tests passed!"
        ;;
    "integration")
        print_info "Running integration tests..."
        cargo test --test "*integration*"
        print_success "Integration tests passed!"
        ;;
    "all")
        print_info "Running all tests..."
        cargo test
        print_success "All tests passed!"
        ;;
    "check")
        print_info "Running cargo check..."
        cargo check
        print_success "Code compiles successfully!"
        ;;
    "fmt")
        print_info "Checking code formatting..."
        cargo fmt --check
        print_success "Code is properly formatted!"
        ;;
    "clippy")
        print_info "Running clippy..."
        cargo clippy -- -D warnings
        print_success "Clippy checks passed!"
        ;;
    "coverage")
        if ! command -v cargo-tarpaulin &> /dev/null; then
            print_warning "cargo-tarpaulin not installed. Install with: cargo install cargo-tarpaulin"
            exit 1
        fi
        print_info "Generating test coverage report..."
        cargo tarpaulin --out Html --output-dir ../target/tarpaulin
        print_success "Coverage report generated at target/tarpaulin/tarpaulin-report.html"
        ;;
    "clean")
        print_info "Cleaning build artifacts..."
        cargo clean
        print_success "Cleaned!"
        ;;
    "watch")
        if ! command -v cargo-watch &> /dev/null; then
            print_warning "cargo-watch not installed. Install with: cargo install cargo-watch"
            print_info "Falling back to manual watch mode..."
            print_info "Run: cargo watch -x test"
            exit 1
        fi
        print_info "Starting test watcher..."
        cargo watch -x test
        ;;
    "help"|*)
        echo "Blenny Test Runner"
        echo ""
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  all          Run all tests (default)"
        echo "  unit         Run only unit tests"
        echo "  integration  Run only integration tests"
        echo "  check        Check that code compiles"
        echo "  fmt          Check code formatting"
        echo "  clippy       Run clippy linter"
        echo "  coverage     Generate test coverage report (requires cargo-tarpaulin)"
        echo "  clean        Clean build artifacts"
        echo "  watch        Watch for changes and run tests (requires cargo-watch)"
        echo "  help         Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0 all          # Run all tests"
        echo "  $0 unit         # Run only unit tests"
        echo "  $0 integration  # Run only integration tests"
        echo "  $0 watch        # Watch mode for TDD"
        ;;
esac