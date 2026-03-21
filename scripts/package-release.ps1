param(
    [string]$TargetTriple = "windows-amd64",
    [string]$BinaryRoot = ""
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$dist = Join-Path $root "dist"
$tmp = Join-Path $dist "tmp"

Push-Location $root

cargo build --release -p api -p worker -p node-agent

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
$agentTmp = Join-Path $tmp "node-agent"
$migrationsTmp = Join-Path $tmp "migrations"

New-Item -ItemType Directory -Force -Path $apiTmp, $workerTmp, $agentTmp, $migrationsTmp | Out-Null

if ($TargetTriple -eq "linux-amd64" -and [string]::IsNullOrWhiteSpace($BinaryRoot)) {
    throw "linux-amd64 artifacts must be packaged from Linux builds. Pass -BinaryRoot with prebuilt Linux binaries or use scripts/package-release.sh on Linux."
}

if ([string]::IsNullOrWhiteSpace($BinaryRoot)) {
    Copy-Item (Join-Path $root "target\release\api.exe") (Join-Path $apiTmp "api.exe")
    Copy-Item (Join-Path $root "target\release\worker.exe") (Join-Path $workerTmp "worker.exe")
    Copy-Item (Join-Path $root "target\release\node-agent.exe") (Join-Path $agentTmp "node-agent.exe")
} else {
    Copy-Item (Join-Path $BinaryRoot "api") (Join-Path $apiTmp "api")
    Copy-Item (Join-Path $BinaryRoot "worker") (Join-Path $workerTmp "worker")
    Copy-Item (Join-Path $BinaryRoot "node-agent") (Join-Path $agentTmp "node-agent")
}

Copy-Item (Join-Path $root "migrations\*") $migrationsTmp -Recurse

if ([string]::IsNullOrWhiteSpace($BinaryRoot)) {
    tar -czf (Join-Path $dist "api-$TargetTriple.tar.gz") -C $apiTmp api.exe
    tar -czf (Join-Path $dist "worker-$TargetTriple.tar.gz") -C $workerTmp worker.exe
    tar -czf (Join-Path $dist "node-agent-$TargetTriple.tar.gz") -C $agentTmp node-agent.exe
} else {
    tar -czf (Join-Path $dist "api-$TargetTriple.tar.gz") -C $apiTmp api
    tar -czf (Join-Path $dist "worker-$TargetTriple.tar.gz") -C $workerTmp worker
    tar -czf (Join-Path $dist "node-agent-$TargetTriple.tar.gz") -C $agentTmp node-agent
}
tar -czf (Join-Path $dist "migrations.tar.gz") -C $migrationsTmp .
tar -czf (Join-Path $dist "web.tar.gz") -C (Join-Path $root "web\dist") .

Pop-Location
