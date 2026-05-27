# Default task when running `just` without arguments
default:
    @just --list

# ── Web Demo (WASM) ───────────────────────────────────────────────────────────

# Build the WASM module and output it to the website directory
build-wasm:
    cd wasm-deslop && wasm-pack build --target web --out-dir ../website/pkg

# Serve the website locally on port 8080
serve-website: build-wasm
    @echo "Starting server at http://localhost:8080 ..."
    python3 -m http.server 8080 -d website

# ── VS Code Extension ─────────────────────────────────────────────────────────

# Install dependencies and compile the VS Code extension
build-vscode:
    cd editors/vscode-deslop && npm install && npm run compile

# Package the VS Code extension into a .vsix file
package-vscode: build-vscode
    cd editors/vscode-deslop && npx vsce package

# ── Core Engine (Rust) ────────────────────────────────────────────────────────

# Build the core CLI and language server
build-core:
    cargo build

# Run tests across the workspace
test:
    cargo test

# Build everything
build-all: build-core build-wasm build-vscode
