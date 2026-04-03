<#
.SYNOPSIS
    Update bb submodules.

.DESCRIPTION
    Initializes and updates git submodules used by bb.
    Without arguments, updates all submodules.
    With a name argument, updates only the specified submodule.

.PARAMETER Name
    Optional submodule name to update: "sparse", "phnt", or "all" (default).

.EXAMPLE
    .\update-submodules.ps1           # Update all submodules
    .\update-submodules.ps1 sparse    # Update only sparse (sdk-api data)
    .\update-submodules.ps1 phnt      # Update only phnt (NT headers)
#>

param(
    [ValidateSet("all", "sparse", "phnt")]
    [string]$Name = "all"
)

$ErrorActionPreference = "Stop"

function Update-Sparse {
    Write-Host "Updating sparse submodule..." -ForegroundColor Cyan
    git submodule update --init crates/bb-sparse/sparse
    if ($LASTEXITCODE -ne 0) { throw "Failed to update sparse submodule" }

    Write-Host "Updating sparse/sdk-api nested submodule..." -ForegroundColor Cyan
    Push-Location crates/bb-sparse/sparse
    git submodule update --init sdk-api
    if ($LASTEXITCODE -ne 0) { Pop-Location; throw "Failed to update sdk-api submodule" }
    Pop-Location

    Write-Host "sparse submodule ready." -ForegroundColor Green
}

function Update-Phnt {
    Write-Host "Updating phnt submodule..." -ForegroundColor Cyan
    git submodule update --init crates/bb-sdk/phnt
    if ($LASTEXITCODE -ne 0) { throw "Failed to update phnt submodule" }

    # The phnt submodule has a nested systeminformer submodule.
    # Only needed if you want to regenerate phnt.h from source.
    $siDir = "crates/bb-sdk/phnt/systeminformer"
    if (-not (Test-Path "$siDir/phnt")) {
        Write-Host "Updating phnt/systeminformer nested submodule..." -ForegroundColor Cyan
        Push-Location crates/bb-sdk/phnt
        git submodule update --init systeminformer
        if ($LASTEXITCODE -ne 0) { Pop-Location; throw "Failed to update systeminformer submodule" }
        Pop-Location
    }

    # Generate phnt.h from the submodule.
    $outPhnt = "crates/bb-sdk/phnt/out/phnt.h"
    if (Test-Path $outPhnt) {
        Write-Host "phnt.h already exists at $outPhnt" -ForegroundColor Green
    } else {
        Write-Host "Generating phnt.h via amalgamate.py..." -ForegroundColor Cyan

        # amalgamate.py downloads cpp-amalgamate.exe via urllib, which can
        # fail silently on some Python versions. Pre-download it with
        # Invoke-WebRequest if it's missing or empty.
        $cppAmalgamate = "crates/bb-sdk/phnt/cpp-amalgamate.exe"
        $downloadUrl = "https://github.com/Felerius/cpp-amalgamate/releases/download/1.0.1/cpp-amalgamate-x86_64-pc-windows-gnu.exe"
        if (-not (Test-Path $cppAmalgamate) -or (Get-Item $cppAmalgamate).Length -eq 0) {
            Write-Host "  Pre-downloading cpp-amalgamate.exe..." -ForegroundColor DarkGray
            if (Test-Path $cppAmalgamate) { Remove-Item $cppAmalgamate }
            Invoke-WebRequest -Uri $downloadUrl -OutFile $cppAmalgamate -UseBasicParsing
        }

        Push-Location crates/bb-sdk/phnt
        py -3 amalgamate.py
        if ($LASTEXITCODE -ne 0) { Pop-Location; throw "amalgamate.py failed" }
        Pop-Location
        Write-Host "phnt.h generated at $outPhnt" -ForegroundColor Green
    }
}

switch ($Name) {
    "all" {
        Update-Sparse
        Write-Host ""
        Update-Phnt
    }
    "sparse" { Update-Sparse }
    "phnt"   { Update-Phnt }
}

Write-Host ""
Write-Host "Done." -ForegroundColor Green
