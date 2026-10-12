# Windows build + test script. Has not yet been executed on Windows.
# Prerequisites: rustup, Node.js 22, Visual Studio 2022 Build Tools (C++), FFmpeg on PATH
#   winget install Rustlang.Rustup OpenJS.NodeJS.LTS Gyan.FFmpeg
param([switch]$Release, [switch]$SkipTests)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

foreach ($tool in 'cargo', 'npm', 'ffmpeg', 'ffprobe') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "$tool not found on PATH. See README.md > Windows setup."
    }
}

function Invoke-Checked([scriptblock]$Block) {
    & $Block
    if ($LASTEXITCODE -ne 0) { throw "Command failed with exit code ${LASTEXITCODE}: $Block" }
}

if (-not $SkipTests) { Invoke-Checked { cargo test --workspace } }

Push-Location apps/desktop
try {
    Invoke-Checked { npm ci }
    if (-not $SkipTests) {
        Invoke-Checked { npm run typecheck }
        Invoke-Checked { npm test }
    }
    if ($Release) {
        Invoke-Checked { npm run tauri build }
    } else {
        Invoke-Checked { npm run tauri build -- --debug --no-bundle }
    }
} finally {
    Pop-Location
}
