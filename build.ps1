Write-Host "Starting build process..." -ForegroundColor Green

# Build the main server binary in release mode
Write-Host "Building server (p2p_server)..."
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error building server!" -ForegroundColor Red
    exit 1
}

# Build all examples in release mode
Write-Host "Building examples..."
cargo build --examples --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error building examples!" -ForegroundColor Red
    exit 1
}

Write-Host "Build completed successfully!" -ForegroundColor Green
Write-Host "Executables are located in:"
Write-Host "- Server: target\release\p2p_server.exe"
Write-Host "- Examples: target\release\examples\"