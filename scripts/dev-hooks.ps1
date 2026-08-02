#Requires -Version 5.1
<#
.SYNOPSIS
    Active les hooks git du dépôt et le gabarit de message de commit.

.DESCRIPTION
    Git n'active jamais un hook versionné tout seul : `.git/hooks/` est local
    et ne se clone pas. Ce script pointe `core.hooksPath` vers `.githooks/`,
    qui est dans le dépôt — donc partagé, revu, et corrigé en un seul endroit.

    Il ne modifie que la configuration git DE CE DÉPÔT. Rien de global, rien
    hors du dossier du projet.

.PARAMETER Desactiver
    Retire les réglages posés par ce script.

.EXAMPLE
    .\scripts\dev-hooks.ps1
    .\scripts\dev-hooks.ps1 -Desactiver
#>

[CmdletBinding()]
param(
    [switch]$Desactiver
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$racine = Split-Path -Parent $PSScriptRoot
Push-Location $racine
try {
    if (-not (Test-Path (Join-Path $racine '.git'))) {
        throw "Ce dossier n'est pas un dépôt git : $racine"
    }

    if ($Desactiver) {
        git config --unset core.hooksPath 2>$null
        git config --unset commit.template 2>$null
        Write-Host ''
        Write-Host '  Hooks et gabarit désactivés pour ce dépôt.' -ForegroundColor Yellow
        Write-Host ''
        return
    }

    git config core.hooksPath .githooks
    git config commit.template .gitmessage

    # Les hooks sont des scripts shell : Git pour Windows les exécute avec son
    # bash embarqué. Le bit d'exécution n'existe pas en NTFS, mais l'index git
    # le porte — on s'assure qu'il y est, sinon le hook est ignoré en silence
    # sur les postes Linux et macOS.
    #
    # `git add --chmod=+x` et non `git update-index --chmod` : le second exige
    # que le fichier soit DÉJÀ dans l'index et échoue sinon.
    foreach ($h in @('commit-msg', 'pre-commit', 'pre-push')) {
        $chemin = ".githooks/$h"
        if (Test-Path $chemin) {
            git add --chmod=+x -- $chemin 2>&1 | Out-Null
        }
    }

    Write-Host ''
    Write-Host '  Hooks activés pour ce dépôt.' -ForegroundColor Green
    Write-Host ''
    Write-Host '    commit-msg   Conventional Commits + portée + référence d''exigence'
    Write-Host '    pre-commit   format, secrets, fichiers interdits      (~1 s)'
    Write-Host '    pre-push     fmt + clippy -D warnings + tests + SEC-02 (2-4 min)'
    Write-Host ''
    Write-Host '  Gabarit de message : .gitmessage'
    Write-Host '  Contournement ponctuel : --no-verify (à justifier en revue).'
    Write-Host ''
}
finally {
    Pop-Location
}
