#Requires -Version 5.1
# PowerShell 5.1 suffit : ce script n'emploie aucune syntaxe propre à la 7.
# Exiger la 7 ajoutait un prérequis sans contrepartie.
<#
.SYNOPSIS
    Compile l'agent Linux depuis Windows, et le dépose dans une distro WSL.

.DESCRIPTION
    On cross-compile depuis Windows plutôt que de builder dans WSL. Pourquoi :
    un seul dépôt, un seul `cargo build`, aucun clone à garder synchrone.

    On n'entre dans WSL que pour EXÉCUTER l'agent. Copier un binaire à travers
    /mnt ne coûte rien ; y compiler un workspace Rust, si — la traduction 9P
    coûte cher sur les milliers de petits fichiers que produit cargo.

    Cible : x86_64-unknown-linux-musl, ELF statique — donc aucune dépendance à
    la glibc de la distro cible, et le même binaire fonctionne d'Alpine à Ubuntu.

.PARAMETER Distro
    Distro WSL où déposer le binaire. Si omis, produit seulement dist\ks-agent.

.PARAMETER Release
    Compiler en release.

.PARAMETER Run
    Exécuter l'agent dans la distro après le dépôt.

.EXAMPLE
    .\scripts\build-agent.ps1 -Distro Debian -Run
#>

[CmdletBinding()]
param(
    [string] $Distro,
    [switch] $Release,
    [switch] $Run
)

$ErrorActionPreference = 'Stop'
$racine = Split-Path -Parent $PSScriptRoot
Push-Location $racine

try {
    $cible = 'x86_64-unknown-linux-musl'
    $profil = if ($Release) { 'release' } else { 'debug' }

    # cargo-zigbuild utilise Zig comme linker multiplateforme : c'est la voie la
    # plus simple pour produire un ELF musl depuis Windows, sans Docker.
    $zigbuild = Get-Command cargo-zigbuild -ErrorAction SilentlyContinue
    $cross    = Get-Command cross -ErrorAction SilentlyContinue

    # `$argsCargo` et non `$args` : ce dernier est une variable AUTOMATIQUE de
    # PowerShell (les arguments non liés). L'écraser passe inaperçu dans un script
    # au niveau racine, et devient un bug silencieux le jour où ce bloc est enrobé
    # dans une fonction. Règle PSScriptAnalyzer : PSAvoidAssignmentToAutomaticVariable.
    if ($zigbuild) {
        Write-Host ('  → cargo zigbuild --target ' + $cible + ' (' + $profil + ')') -ForegroundColor Cyan
        $argsCargo = @('zigbuild', '-p', 'ks-agent-linux', '--target', $cible)
        if ($Release) { $argsCargo += '--release' }
        & cargo @argsCargo
    }
    elseif ($cross) {
        Write-Host ('  → cross build --target ' + $cible + ' (' + $profil + ')') -ForegroundColor Cyan
        $argsCargo = @('build', '-p', 'ks-agent-linux', '--target', $cible)
        if ($Release) { $argsCargo += '--release' }
        & cross @argsCargo
    }
    else {
        throw @'
Aucun outil de cross-compilation trouvé.

  Option 1, recommandée (pas de Docker) :
      cargo install cargo-zigbuild
      winget install zig.zig

  Option 2, si Docker est déjà installé :
      cargo install cross

Détail : docs/05-ENVIRONNEMENT-DE-DEV.md
'@
    }

    if ($LASTEXITCODE -ne 0) { throw 'La compilation a échoué.' }

    $source = Join-Path $racine "target\$cible\$profil\ks-agent"
    if (-not (Test-Path $source)) { throw "Binaire introuvable : $source" }

    New-Item -ItemType Directory -Force -Path (Join-Path $racine 'dist') | Out-Null
    $dist = Join-Path $racine 'dist\ks-agent'
    Copy-Item $source $dist -Force

    $taille = [math]::Round((Get-Item $dist).Length / 1MB, 2)
    Write-Host ('  ✓ dist\ks-agent — ' + $taille + ' Mo, ELF statique musl') -ForegroundColor Green

    if (-not $Distro) {
        Write-Host ''
        Write-Host '  Pour déposer dans une distro :  .\scripts\build-agent.ps1 -Distro Debian -Run' -ForegroundColor DarkGray
        Write-Host ('  Distros disponibles : ' + ((wsl --list --quiet) -join ', ')) -ForegroundColor DarkGray
        return
    }

    # Le chemin Windows vu depuis WSL. `wslpath` fait la conversion proprement,
    # y compris quand le dépôt est sur un lecteur autre que C:.
    $chemin = wsl -d $Distro -- wslpath -a ($dist -replace '\\', '/')
    Write-Host ('  → dépôt dans ' + $Distro + ' : /tmp/ks-agent') -ForegroundColor Cyan
    wsl -d $Distro -- sh -c "cp '$chemin' /tmp/ks-agent && chmod +x /tmp/ks-agent"
    if ($LASTEXITCODE -ne 0) { throw "Échec du dépôt dans $Distro." }
    Write-Host '  ✓ déposé' -ForegroundColor Green

    if ($Run) {
        Write-Host ''
        Write-Host ('  ── exécution dans ' + $Distro + ' ──') -ForegroundColor DarkGray
        wsl -d $Distro -- /tmp/ks-agent
    }
}
finally {
    Pop-Location
}
