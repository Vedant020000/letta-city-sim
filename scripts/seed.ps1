param(
    [string]$DbUser = "sim",
    [string]$DbName = "letta_city_sim"
)

$ErrorActionPreference = "Stop"

$seedOrderFile = Join-Path $PSScriptRoot "seed-order.txt"
if (-Not (Test-Path $seedOrderFile)) {
    throw "seed-order.txt not found at $seedOrderFile"
}

$seedFiles = Get-Content $seedOrderFile |
    Where-Object { $_ -and $_ -notmatch '^\s*#' } |
    ForEach-Object { "seed/$($_.Trim())" }

foreach ($file in $seedFiles) {
    Write-Host "Applying $file ..."
    Get-Content $file -Raw | docker compose exec -T db psql -U $DbUser -d $DbName -f -
    if ($LASTEXITCODE -ne 0) {
        throw "Failed while applying $file"
    }
}

Write-Host "Seeding complete."
