# PulseCut Windows development notes.
# This script was written from the Linux slice. It has not been run on Windows 11.
# It does not install system software. It prints the tools the project expects
# and, if they are already present, runs the same checks as the Linux workflow.

$ErrorActionPreference = "Stop"

Write-Host "Expected tools:"
Write-Host "  rustup toolchain 1.99.0 (see rust-toolchain.toml)"
Write-Host "  Node.js 22 and pnpm 10"
Write-Host "  FFmpeg on PATH with libx264 and an AAC encoder"
Write-Host "  Visual Studio C++ build tools, WebView2 (Tauri 2 Windows prerequisites)"
Write-Host ""
Write-Host "Not done from the first Linux slice: tauri build, an MSI/NSIS installer, or a GUI acceptance pass."

function Assert-Command($name) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        throw "Missing '$name'. Install it, then re-run this script."
    }
}

Assert-Command cargo
Assert-Command node
Assert-Command pnpm
Assert-Command ffmpeg

ffmpeg -hide_banner -encoders | Select-String "libx264" | Out-Host
if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed" }

Push-Location (Split-Path $PSScriptRoot -Parent)
cargo test --workspace
if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }
Pop-Location

Push-Location (Join-Path (Split-Path $PSScriptRoot -Parent) "apps/desktop")
pnpm install --frozen-lockfile
pnpm test
pnpm exec tsc --noEmit
Pop-Location

Write-Host "Checks finished. Start the app with: cd apps/desktop; pnpm tauri dev"
