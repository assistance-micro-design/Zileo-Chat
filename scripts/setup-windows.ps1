# Zileo-Chat-3 - Windows Setup Script
# Execute avec: powershell -ExecutionPolicy Bypass -File setup-windows.ps1

param(
    [switch]$SkipPrerequisites,
    [switch]$BuildOnly,
    [switch]$DevMode
)

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Zileo-Chat-3 - Windows Setup Script  " -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Fonction pour verifier si une commande existe
function Test-CommandExists {
    param($Command)
    $null -ne (Get-Command $Command -ErrorAction SilentlyContinue)
}

# Fonction pour afficher le statut
function Write-Status {
    param($Message, $Status)
    if ($Status -eq "OK") {
        Write-Host "[OK] " -ForegroundColor Green -NoNewline
    } elseif ($Status -eq "MISSING") {
        Write-Host "[MISSING] " -ForegroundColor Red -NoNewline
    } elseif ($Status -eq "INFO") {
        Write-Host "[INFO] " -ForegroundColor Yellow -NoNewline
    }
    Write-Host $Message
}

# ============================================
# ETAPE 1: Verification des prerequisites
# ============================================

if (-not $SkipPrerequisites -and -not $BuildOnly) {
    Write-Host "`n--- Verification des prerequisites ---" -ForegroundColor Yellow

    $missingPrereqs = @()

    # Verifier Rust
    if (Test-CommandExists "rustc") {
        $rustVersion = rustc --version
        Write-Status "Rust: $rustVersion" "OK"
    } else {
        Write-Status "Rust non installe" "MISSING"
        $missingPrereqs += "rust"
    }

    # Verifier Node.js
    if (Test-CommandExists "node") {
        $nodeVersion = node --version
        $nodeMajor = [int]($nodeVersion -replace 'v(\d+)\..*', '$1')
        if ($nodeMajor -ge 20) {
            Write-Status "Node.js: $nodeVersion" "OK"
        } else {
            Write-Status "Node.js $nodeVersion (version 20+ requise)" "MISSING"
            $missingPrereqs += "node"
        }
    } else {
        Write-Status "Node.js non installe" "MISSING"
        $missingPrereqs += "node"
    }

    # Verifier Git
    if (Test-CommandExists "git") {
        $gitVersion = git --version
        Write-Status "Git: $gitVersion" "OK"
    } else {
        Write-Status "Git non installe" "MISSING"
        $missingPrereqs += "git"
    }

    # Verifier Visual Studio Build Tools (via cl.exe)
    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vsWhere) {
        $vsPath = & $vsWhere -latest -property installationPath
        if ($vsPath) {
            Write-Status "Visual Studio Build Tools: $vsPath" "OK"
        } else {
            Write-Status "Visual Studio Build Tools non trouve" "MISSING"
            $missingPrereqs += "vs-buildtools"
        }
    } else {
        Write-Status "Visual Studio Build Tools non installe" "MISSING"
        $missingPrereqs += "vs-buildtools"
    }

    # Installer les prerequisites manquants
    if ($missingPrereqs.Count -gt 0) {
        Write-Host "`n--- Installation des prerequisites manquants ---" -ForegroundColor Yellow

        # Verifier si winget est disponible
        if (-not (Test-CommandExists "winget")) {
            Write-Host "ERREUR: winget non disponible. Installez manuellement:" -ForegroundColor Red
            Write-Host "  - Rust: https://rustup.rs/"
            Write-Host "  - Node.js: https://nodejs.org/"
            Write-Host "  - Visual Studio Build Tools: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
            exit 1
        }

        foreach ($prereq in $missingPrereqs) {
            switch ($prereq) {
                "rust" {
                    Write-Status "Installation de Rust..." "INFO"
                    winget install Rustlang.Rustup --silent --accept-package-agreements
                    # Configurer le toolchain MSVC
                    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
                    rustup default stable-msvc
                }
                "node" {
                    Write-Status "Installation de Node.js LTS..." "INFO"
                    winget install OpenJS.NodeJS.LTS --silent --accept-package-agreements
                }
                "git" {
                    Write-Status "Installation de Git..." "INFO"
                    winget install Git.Git --silent --accept-package-agreements
                }
                "vs-buildtools" {
                    Write-Status "Installation de Visual Studio Build Tools..." "INFO"
                    Write-Host "  Cela peut prendre plusieurs minutes..." -ForegroundColor Gray
                    winget install Microsoft.VisualStudio.2022.BuildTools --silent --accept-package-agreements `
                        --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
                }
            }
        }

        Write-Host "`n" -ForegroundColor Yellow
        Write-Host "IMPORTANT: Fermez et rouvrez PowerShell pour appliquer les changements PATH." -ForegroundColor Yellow
        Write-Host "Puis relancez ce script avec: .\setup-windows.ps1 -SkipPrerequisites" -ForegroundColor Yellow
        exit 0
    }

    Write-Host "`nTous les prerequisites sont installes!" -ForegroundColor Green
}

# ============================================
# ETAPE 2: Configuration Rust
# ============================================

Write-Host "`n--- Configuration Rust ---" -ForegroundColor Yellow

$rustToolchain = rustup show active-toolchain 2>$null
if ($rustToolchain -match "msvc") {
    Write-Status "Toolchain MSVC actif" "OK"
} else {
    Write-Status "Configuration du toolchain MSVC..." "INFO"
    rustup default stable-msvc
}

# ============================================
# ETAPE 3: Installation des dependances
# ============================================

Write-Host "`n--- Installation des dependances npm ---" -ForegroundColor Yellow

if (Test-Path "package.json") {
    npm ci
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERREUR: npm ci a echoue" -ForegroundColor Red
        exit 1
    }
    Write-Status "Dependances npm installees" "OK"
} else {
    Write-Host "ERREUR: package.json non trouve. Etes-vous dans le bon repertoire?" -ForegroundColor Red
    exit 1
}

# ============================================
# ETAPE 4: Validation du projet
# ============================================

Write-Host "`n--- Validation du projet ---" -ForegroundColor Yellow

Write-Status "Verification TypeScript/Svelte..." "INFO"
npm run check
if ($LASTEXITCODE -ne 0) {
    Write-Host "ATTENTION: Des erreurs TypeScript ont ete detectees" -ForegroundColor Yellow
}

Write-Status "Verification ESLint..." "INFO"
npm run lint
if ($LASTEXITCODE -ne 0) {
    Write-Host "ATTENTION: Des erreurs ESLint ont ete detectees" -ForegroundColor Yellow
}

# ============================================
# ETAPE 5: Build ou Dev
# ============================================

if ($DevMode) {
    Write-Host "`n--- Lancement en mode developpement ---" -ForegroundColor Yellow
    Write-Host "L'application va s'ouvrir. Ctrl+C pour arreter." -ForegroundColor Gray
    npm run tauri dev
} else {
    Write-Host "`n--- Build de l'application ---" -ForegroundColor Yellow
    Write-Host "Premiere compilation: 10-15 minutes" -ForegroundColor Gray
    Write-Host "Compilations suivantes: 2-5 minutes" -ForegroundColor Gray
    Write-Host ""

    npm run tauri build

    if ($LASTEXITCODE -eq 0) {
        Write-Host "`n========================================" -ForegroundColor Green
        Write-Host "  BUILD REUSSI!" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Green

        $msiPath = "src-tauri\target\release\bundle\msi"
        if (Test-Path $msiPath) {
            Write-Host "`nFichiers generes:" -ForegroundColor Cyan
            Get-ChildItem $msiPath -Filter "*.msi" | ForEach-Object {
                Write-Host "  - $($_.FullName)" -ForegroundColor White
            }

            Write-Host "`nPour installer:" -ForegroundColor Yellow
            Write-Host "  explorer `"$msiPath`"" -ForegroundColor Gray
            Write-Host "  Double-cliquez sur le fichier .msi" -ForegroundColor Gray
        }
    } else {
        Write-Host "`nERREUR: Le build a echoue" -ForegroundColor Red
        exit 1
    }
}

Write-Host "`nTermine!" -ForegroundColor Green
