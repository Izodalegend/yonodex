# Yonodex Release Signing Script
# Generates SHA-256 checksums and GPG detached signatures for installers.
# Usage: .\sign-release.ps1 -FilePath "path\to\Yonodex_x.x.x_x64-setup.exe"

param (
    [Parameter(Mandatory=$true)]
    [string]$FilePath
)

if (-not (Test-Path $FilePath)) {
    Write-Error "File not found: $FilePath"
    exit 1
}

$fileName = Split-Path $FilePath -Leaf
$directory = Split-Path $FilePath -Parent
$gpgKey = "dumizo@yahoo.com"

Write-Host "Processing $fileName..." -ForegroundColor Cyan

# 1. Generate SHA-256 checksum
Write-Host "Generating SHA-256 checksum..."
$hash = (Get-FileHash -Algorithm SHA256 -Path $FilePath).Hash.ToLower()
$checksumFile = "$FilePath.sha256"
"$hash  $fileName" | Out-File -FilePath $checksumFile -Encoding ascii
Write-Host "Created: $checksumFile" -ForegroundColor Green

# 2. Generate GPG detached signature for the binary
Write-Host "Signing binary with GPG..."
gpg --yes --armor --detach-sign --local-user $gpgKey --output "$FilePath.asc" $FilePath
if ($LASTEXITCODE -ne 0) {
    Write-Error "GPG signing failed for the binary."
    exit 1
}
Write-Host "Created: $FilePath.asc" -ForegroundColor Green

# 3. Generate GPG detached signature for the checksum file
Write-Host "Signing checksum file with GPG..."
gpg --yes --armor --detach-sign --local-user $gpgKey --output "$checksumFile.asc" $checksumFile
if ($LASTEXITCODE -ne 0) {
    Write-Error "GPG signing failed for the checksum."
    exit 1
}
Write-Host "Created: $checksumFile.asc" -ForegroundColor Green

Write-Host "Release signing complete!" -ForegroundColor Cyan