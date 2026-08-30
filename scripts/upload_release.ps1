param()
$TAG = "v0.2.1"
$REPO = "cLLeB/bible-app"
$MAX_RETRIES = 10        # retry each file up to 10 times
$RETRY_WAIT  = 20        # seconds to wait between retries

Write-Host ""
Write-Host "=== Bible App Release Uploader ===" -ForegroundColor Cyan
Write-Host "Tag: $TAG  Repo: $REPO"
Write-Host ""

$TOKEN = gh auth token
if (-not $TOKEN) {
    Write-Host "ERROR: Not logged in. Run: gh auth login" -ForegroundColor Red
    exit 1
}

# Must use numeric REST ID (not GraphQL node ID) for the upload URL
$releaseJson = gh api "repos/$REPO/releases/tags/$TAG" | ConvertFrom-Json
$RELEASE_ID = $releaseJson.id
$uploadedNames = @($releaseJson.assets | ForEach-Object { $_.name })

Write-Host "Release ID: $RELEASE_ID"
Write-Host "Already uploaded: $($uploadedNames.Count) file(s)"
Write-Host ""

$files = Get-ChildItem -Path "installers" -Include "*.exe","*.msi" -Recurse | Sort-Object Length

$total = $files.Count
$done = 0

$responseFile = [System.IO.Path]::GetTempFileName()

foreach ($file in $files) {
    $done++
    $sizeMB = [math]::Round($file.Length / 1MB, 1)
    $label = "[$done/$total]"

    if ($uploadedNames -contains $file.Name) {
        Write-Host "$label SKIP  $($file.Name) ($sizeMB MB) - already uploaded" -ForegroundColor Green
        continue
    }

    $url = "https://uploads.github.com/repos/$REPO/releases/$RELEASE_ID/assets?name=$($file.Name)"
    $attempt = 0
    $success = $false

    while (-not $success -and $attempt -lt $MAX_RETRIES) {
        $attempt++
        $attemptLabel = if ($attempt -eq 1) { "" } else { " (attempt $attempt/$MAX_RETRIES)" }

        Write-Host ""
        Write-Host "$label UPLOADING  $($file.Name) ($sizeMB MB)$attemptLabel" -ForegroundColor Yellow
        Write-Host "------------------------------------------------------------"

        curl.exe `
            --progress-bar `
            -H "Authorization: token $TOKEN" `
            -H "Content-Type: application/octet-stream" `
            --data-binary "@$($file.FullName)" `
            -o $responseFile `
            "$url"

        $exitCode = $LASTEXITCODE
        $response = Get-Content $responseFile -Raw -ErrorAction SilentlyContinue

        if ($exitCode -eq 0 -and $response -match '"id"') {
            Write-Host "  DONE" -ForegroundColor Green
            $uploadedNames += $file.Name
            $success = $true
        } else {
            Write-Host "  FAILED (exit code $exitCode)" -ForegroundColor Red
            if ($response) { Write-Host "  Response: $response" -ForegroundColor DarkRed }

            if ($attempt -lt $MAX_RETRIES) {
                Write-Host "  Waiting $RETRY_WAIT seconds then retrying..." -ForegroundColor Yellow
                Start-Sleep -Seconds $RETRY_WAIT
            } else {
                Write-Host "  Gave up after $MAX_RETRIES attempts. Re-run the script later to retry." -ForegroundColor Red
            }
        }
    }
}

Remove-Item $responseFile -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "=== Finished ===" -ForegroundColor Cyan
Write-Host "https://github.com/$REPO/releases/tag/$TAG"
