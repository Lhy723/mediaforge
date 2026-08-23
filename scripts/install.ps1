[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$repo = if ($env:MEDIAFORGE_REPO) { $env:MEDIAFORGE_REPO } else { "Lhy723/mediaforge" }
$version = if ($env:MEDIAFORGE_VERSION) { $env:MEDIAFORGE_VERSION } else { "latest" }
$installDir = if ($env:MEDIAFORGE_INSTALL_DIR) {
    $env:MEDIAFORGE_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA "MediaForge\bin"
}

$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
if ($env:MEDIAFORGE_TARGET) {
    $target = $env:MEDIAFORGE_TARGET
} elseif ($architecture -eq "X64") {
    $target = "x86_64-pc-windows-msvc"
} else {
    throw "MediaForge does not publish a Windows asset for $architecture yet."
}

if ($version -eq "latest") {
    $asset = "mediaforge-$target.zip"
    $url = "https://github.com/$repo/releases/latest/download/$asset"
} else {
    $asset = "mediaforge-$version-$target.zip"
    $url = "https://github.com/$repo/releases/download/$version/$asset"
}

$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("mediaforge-install-" + [guid]::NewGuid().ToString("N"))
$archive = Join-Path $tempDir $asset

try {
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $archive
    Expand-Archive -Path $archive -DestinationPath $tempDir -Force

    $binary = Join-Path $tempDir "media.exe"
    if (!(Test-Path -LiteralPath $binary)) {
        throw "The downloaded archive did not contain media.exe."
    }

    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item -Force -LiteralPath $binary -Destination (Join-Path $installDir "media.exe")

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathEntries = @()
    if ($userPath) {
        $pathEntries = @($userPath -split ';' | Where-Object { $_ })
    }
    if ($pathEntries -notcontains $installDir) {
        $newPath = (($pathEntries + $installDir) -join ';')
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    }
    $env:Path = "$installDir;$env:Path"

    Write-Host "MediaForge installed to $(Join-Path $installDir 'media.exe')"
    Write-Host "Open a new PowerShell window if the media command is not found."

    if (!(Get-Command ffmpeg -ErrorAction SilentlyContinue) -or !(Get-Command ffprobe -ErrorAction SilentlyContinue)) {
        Write-Warning "FFmpeg and FFprobe were not found on PATH. Install them before processing media (for example: choco install ffmpeg)."
    }
    Write-Host "Try: media.exe capabilities --json"
} finally {
    if (Test-Path -LiteralPath $tempDir) {
        Remove-Item -Recurse -Force -LiteralPath $tempDir
    }
}
