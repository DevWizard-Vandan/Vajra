# Trinity Cluster Demo Script
# Launches 3 Vajra nodes and demonstrates leader election + failover

param(
    [switch]$Clean = $false
)

$ErrorActionPreference = "Stop"

Write-Host "=== Vajra Trinity Demo ===" -ForegroundColor Cyan

# Paths
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$Binary = "$ProjectRoot\target\release\vajra.exe"
$ConfigDir = "$ProjectRoot\configs"
$DataDir = "$ProjectRoot\data"

# Clean previous data if requested
if ($Clean) {
    Write-Host "Cleaning previous data..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force "$DataDir" -ErrorAction SilentlyContinue
}

# Ensure data directories exist
New-Item -ItemType Directory -Force -Path "$DataDir\node_1" | Out-Null
New-Item -ItemType Directory -Force -Path "$DataDir\node_2" | Out-Null
New-Item -ItemType Directory -Force -Path "$DataDir\node_3" | Out-Null

# Build release binary
Write-Host "`n[1/5] Building release binary..." -ForegroundColor Green
Push-Location $ProjectRoot
cargo build --release -p vajra-server 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}
Pop-Location

Write-Host "Build successful!" -ForegroundColor Green

# Start nodes
Write-Host "`n[2/5] Starting nodes..." -ForegroundColor Green

$Node1 = Start-Process -FilePath $Binary -ArgumentList "--node-id 1 --listen 127.0.0.1:50051 --data-dir $DataDir\node_1" -PassThru -WindowStyle Hidden
$Node2 = Start-Process -FilePath $Binary -ArgumentList "--node-id 2 --listen 127.0.0.1:50052 --data-dir $DataDir\node_2" -PassThru -WindowStyle Hidden
$Node3 = Start-Process -FilePath $Binary -ArgumentList "--node-id 3 --listen 127.0.0.1:50053 --data-dir $DataDir\node_3" -PassThru -WindowStyle Hidden

Write-Host "Node 1 PID: $($Node1.Id)" -ForegroundColor Cyan
Write-Host "Node 2 PID: $($Node2.Id)" -ForegroundColor Cyan
Write-Host "Node 3 PID: $($Node3.Id)" -ForegroundColor Cyan

# Wait for leader election
Write-Host "`n[3/5] Waiting for leader election (5 seconds)..." -ForegroundColor Green
Start-Sleep -Seconds 5

# Check if nodes are running
$Running = @($Node1, $Node2, $Node3) | Where-Object { !$_.HasExited }
Write-Host "Running nodes: $($Running.Count)" -ForegroundColor Cyan

if ($Running.Count -lt 3) {
    Write-Host "Warning: Not all nodes are running!" -ForegroundColor Yellow
}

# Simulate failover
Write-Host "`n[4/5] Killing Node 1 (simulating leader failure)..." -ForegroundColor Yellow
Stop-Process -Id $Node1.Id -Force -ErrorAction SilentlyContinue

Write-Host "Waiting for new leader election (3 seconds)..." -ForegroundColor Yellow
Start-Sleep -Seconds 3

# Check remaining nodes
$Remaining = @($Node2, $Node3) | Where-Object { !$_.HasExited }
Write-Host "Remaining nodes: $($Remaining.Count)" -ForegroundColor Cyan

# Cleanup
Write-Host "`n[5/5] Cleaning up..." -ForegroundColor Green
Stop-Process -Id $Node2.Id -Force -ErrorAction SilentlyContinue
Stop-Process -Id $Node3.Id -Force -ErrorAction SilentlyContinue

Write-Host "`n=== Trinity Demo Complete ===" -ForegroundColor Cyan
Write-Host "Check logs for 'Candidate -> Leader' transition!" -ForegroundColor Yellow
