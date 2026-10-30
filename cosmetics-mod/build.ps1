$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    $cargoExecutable = if ($cargoCommand) { $cargoCommand.Source } else { $null }
    if (-not $cargoExecutable) {
        $localCargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
        if (Test-Path -LiteralPath $localCargo) { $cargoExecutable = $localCargo }
    }
    if (-not $cargoExecutable) { throw 'Install Rust with the Windows MSVC toolchain before building Aster Cosmetics.' }
    & $cargoExecutable test --manifest-path native/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    & $cargoExecutable build --release --manifest-path native/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw 'Rust native build failed.' }
    $localJdk = Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot '.tooling\java') -Directory -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($localJdk) { $env:JAVA_HOME = $localJdk.FullName }
    if (-not $env:JAVA_HOME -and -not (Get-Command java -ErrorAction SilentlyContinue)) { throw 'Install JDK 21 or set JAVA_HOME before building Aster Cosmetics.' }
    $env:GRADLE_USER_HOME = Join-Path $PSScriptRoot '.tooling\gradle-cache'
    if (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'gradlew.bat')) { & '.\gradlew.bat' build --no-daemon --console=plain }
    elseif (Test-Path -LiteralPath (Join-Path $PSScriptRoot '.tooling\gradle-8.10.2\bin\gradle.bat')) { & '.\.tooling\gradle-8.10.2\bin\gradle.bat' build --no-daemon --console=plain }
    else { throw 'Gradle wrapper is missing. Install Gradle 8.10.2 and run gradle wrapper --gradle-version 8.10.2.' }
    if ($LASTEXITCODE -ne 0) { throw "Gradle failed with exit code $LASTEXITCODE" }
} finally { Pop-Location }
