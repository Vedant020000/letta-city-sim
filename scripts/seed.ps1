param(
    [string]$DbUser = "sim",
    [string]$DbName = "letta_city_sim"
)

$ErrorActionPreference = "Stop"

$seedFiles = @(
    "seed/locations.sql",
    "seed/adjacency.sql",
    "seed/objects.sql",
    "seed/agents.sql"
)

foreach ($file in $seedFiles) {
    Write-Host "Applying $file ..."
    Get-Content $file -Raw | docker compose exec -T db psql -U $DbUser -d $DbName -f -
    if ($LASTEXITCODE -ne 0) {
        throw "Failed while applying $file"
    }
}

Write-Host "Seeding complete."
