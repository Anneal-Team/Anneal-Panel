param(
    [string]$TargetTriple = "windows-amd64",
    [string]$BinaryRoot = "",
    [string]$MihomoBinary = ""
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$dist = Join-Path $root "dist"
$tmp = Join-Path $dist "tmp"

Push-Location $root

cargo build --release -p api -p worker -p annealctl

Push-Location (Join-Path $root "web")
npm run build
Pop-Location

New-Item -ItemType Directory -Force -Path $dist | Out-Null
if (Test-Path $tmp) {
    Remove-Item $tmp -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

$apiTmp = Join-Path $tmp "api"
$workerTmp = Join-Path $tmp "worker"
$ctlTmp = Join-Path $tmp "annealctl"
$migrationsTmp = Join-Path $tmp "migrations"

New-Item -ItemType Directory -Force -Path $apiTmp, $workerTmp, $ctlTmp, $migrationsTmp | Out-Null

if ($TargetTriple -eq "linux-amd64" -and [string]::IsNullOrWhiteSpace($BinaryRoot)) {
    throw "linux-amd64 artifacts must be packaged from Linux builds. Pass -BinaryRoot with prebuilt Linux binaries or use scripts/package-release.sh on Linux."
}

if ([string]::IsNullOrWhiteSpace($BinaryRoot)) {
    Copy-Item (Join-Path $root "target\release\api.exe") (Join-Path $apiTmp "api.exe")
    Copy-Item (Join-Path $root "target\release\worker.exe") (Join-Path $workerTmp "worker.exe")
    Copy-Item (Join-Path $root "target\release\annealctl.exe") (Join-Path $ctlTmp "annealctl.exe")
} else {
    Copy-Item (Join-Path $BinaryRoot "api") (Join-Path $apiTmp "api")
    Copy-Item (Join-Path $BinaryRoot "worker") (Join-Path $workerTmp "worker")
    Copy-Item (Join-Path $BinaryRoot "annealctl") (Join-Path $ctlTmp "annealctl")
}

if (-not [string]::IsNullOrWhiteSpace($MihomoBinary)) {
    $runtimeTmp = Join-Path $tmp "runtime"
    New-Item -ItemType Directory -Force -Path $runtimeTmp | Out-Null
    Copy-Item $MihomoBinary (Join-Path $runtimeTmp "mihomo")
    tar -czf (Join-Path $dist "mihomo-$TargetTriple.tar.gz") -C $runtimeTmp mihomo
}

Copy-Item (Join-Path $root "migrations\*") $migrationsTmp -Recurse

if ([string]::IsNullOrWhiteSpace($BinaryRoot)) {
    tar -czf (Join-Path $dist "api-$TargetTriple.tar.gz") -C $apiTmp api.exe
    tar -czf (Join-Path $dist "worker-$TargetTriple.tar.gz") -C $workerTmp worker.exe
    tar -czf (Join-Path $dist "annealctl-$TargetTriple.tar.gz") -C $ctlTmp annealctl.exe
} else {
    tar -czf (Join-Path $dist "api-$TargetTriple.tar.gz") -C $apiTmp api
    tar -czf (Join-Path $dist "worker-$TargetTriple.tar.gz") -C $workerTmp worker
    tar -czf (Join-Path $dist "annealctl-$TargetTriple.tar.gz") -C $ctlTmp annealctl
}
tar -czf (Join-Path $dist "migrations.tar.gz") -C $migrationsTmp .
tar -czf (Join-Path $dist "web.tar.gz") -C (Join-Path $root "web\dist") .

Pop-Location
