.PHONY: build test release

# Build CLI for current platform
build:
	cargo build -p memento --features cli --release

# Run tests
test:
	cargo test -p memento

# Release build (same as build, outputs to target/release/memento)
release: build
	@echo "Binary: target/release/memento"

# Note: Windows cross-compilation from macOS is not supported locally
# due to DuckDB's bundled C++ compilation requiring MSVC.
# Use GitHub Actions (see .github/workflows/build.yml) for Windows builds.
