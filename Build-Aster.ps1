$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    & "$PSScriptRoot/cosmetics-mod/build.ps1"
    if ($LASTEXITCODE -ne 0) { throw 'Cosmetic build failed' }
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) { throw 'Rust formatting check failed' }
    cargo test
    if ($LASTEXITCODE -ne 0) { throw 'Launcher tests failed' }
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw 'Launcher build failed' }
    New-Item -ItemType Directory -Force -Path "$PSScriptRoot/release" | Out-Null
    Copy-Item -LiteralPath "$PSScriptRoot/target/release/Aster-Launcher.exe" -Destination "$PSScriptRoot/release/Aster-Launcher.exe" -Force
    Write-Host 'Built release/Aster-Launcher.exe'
} finally { Pop-Location }
