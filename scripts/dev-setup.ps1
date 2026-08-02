#Requires -Version 5.1
# PowerShell 5.1 et non 7, volontairement : ce script vérifie l'outillage d'un
# poste neuf. Exiger PowerShell 7 en ferait un script qui ne peut pas s'exécuter
# tant que l'outillage est incomplet — c'est-à-dire précisément quand on en a
# besoin. 5.1 est présent sur tout Windows ; PowerShell 7 figure donc parmi ce
# que ce script CONTRÔLE, pas parmi ce qu'il exige.
<#
.SYNOPSIS
    Vérifie que l'environnement de développement Keystone est complet.

.DESCRIPTION
    CE SCRIPT NE MODIFIE RIEN. Il vérifie et rapporte.

    C'est cohérent avec le principe P2 du projet : même les scripts de
    développement montrent avant de faire. Si quelque chose manque, le script
    affiche la commande d'installation — et te laisse la lancer.

.EXAMPLE
    .\scripts\dev-setup.ps1
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'
$script:Manquants = @()

function Test-Outil {
    param(
        [Parameter(Mandatory)] [string] $Nom,
        [Parameter(Mandatory)] [string] $Commande,
        [Parameter(Mandatory)] [string] $Installation,
        [string] $VersionArg = '--version',
        [switch] $Optionnel
    )

    $exe = Get-Command $Commande -ErrorAction SilentlyContinue
    if ($exe) {
        $version = & $Commande $VersionArg 2>&1 | Select-Object -First 1
        Write-Host ('  [ok]   ' + $Nom.PadRight(24)) -NoNewline -ForegroundColor Green
        Write-Host $version -ForegroundColor DarkGray
        return $true
    }

    $etiquette = if ($Optionnel) { '[opt]  ' } else { '[abs]  ' }
    $couleur   = if ($Optionnel) { 'Yellow' } else { 'Red' }
    Write-Host ($etiquette + $Nom.PadRight(24)) -NoNewline -ForegroundColor $couleur
    Write-Host $Installation -ForegroundColor DarkGray
    if (-not $Optionnel) { $script:Manquants += $Nom }
    return $false
}

Write-Host ''
Write-Host '  KEYSTONE — vérification de l''environnement de développement' -ForegroundColor Cyan
Write-Host '  Ce script ne modifie rien.' -ForegroundColor DarkGray
Write-Host ''

# ─── Chaîne Rust ───────────────────────────────────────────────────────────
Write-Host '  Chaîne Rust' -ForegroundColor White
Test-Outil -Nom 'rustc'   -Commande 'rustc'   -Installation 'winget install Rustlang.Rustup' | Out-Null
Test-Outil -Nom 'cargo'   -Commande 'cargo'   -Installation 'winget install Rustlang.Rustup' | Out-Null
Test-Outil -Nom 'rustup'  -Commande 'rustup'  -Installation 'winget install Rustlang.Rustup' | Out-Null

if (Get-Command rustup -ErrorAction SilentlyContinue) {
    $cibles = rustup target list --installed
    foreach ($cible in @('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-musl')) {
        if ($cibles -contains $cible) {
            Write-Host ('  [ok]   cible ' + $cible) -ForegroundColor Green
        } else {
            Write-Host ('  [abs]  cible ' + $cible.PadRight(30)) -NoNewline -ForegroundColor Red
            Write-Host ('rustup target add ' + $cible) -ForegroundColor DarkGray
            $script:Manquants += ('cible ' + $cible)
        }
    }
    foreach ($c in @('rustfmt', 'clippy')) {
        if ((rustup component list --installed) -match $c) {
            Write-Host ('  [ok]   composant ' + $c) -ForegroundColor Green
        } else {
            Write-Host ('  [abs]  composant ' + $c.PadRight(26)) -NoNewline -ForegroundColor Red
            Write-Host ('rustup component add ' + $c) -ForegroundColor DarkGray
            $script:Manquants += ('composant ' + $c)
        }
    }
}

# ─── Linker MSVC ───────────────────────────────────────────────────────────
# La cible du produit est -msvc, pas -gnu : le broker tape dans WMI, DPAPI-NG,
# WFP et l'API TPM. Un binaire -gnu n'est pas le binaire qu'on livre.
Write-Host ''
Write-Host '  Outils de compilation Microsoft' -ForegroundColor White
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path $vswhere) {
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion 2>$null
    if ($vs) {
        Write-Host ('  [ok]   outils C++ MSVC        ' + $vs) -ForegroundColor Green
    } else {
        Write-Host '  [abs]  outils C++ MSVC        ' -NoNewline -ForegroundColor Red
        Write-Host 'installeur VS → « Développement Desktop en C++ » + SDK Windows' -ForegroundColor DarkGray
        $script:Manquants += 'outils C++ MSVC'
    }
} else {
    Write-Host '  [abs]  Visual Studio Build Tools' -NoNewline -ForegroundColor Red
    Write-Host '  winget install Microsoft.VisualStudio.2022.BuildTools' -ForegroundColor DarkGray
    $script:Manquants += 'Visual Studio Build Tools'
}

# ─── Cross-compilation de l'agent Linux ────────────────────────────────────
Write-Host ''
Write-Host '  Cross-compilation de l''agent Linux' -ForegroundColor White
$zb = Test-Outil -Nom 'cargo-zigbuild' -Commande 'cargo-zigbuild' -Installation 'cargo install cargo-zigbuild' -Optionnel
$zig = Test-Outil -Nom 'zig' -Commande 'zig' -Installation 'winget install zig.zig' -Optionnel -VersionArg 'version'
if (-not ($zb -and $zig)) {
    Write-Host '         Alternative si Docker est déjà installé : cargo install cross' -ForegroundColor DarkGray
}

# ─── Le reste ──────────────────────────────────────────────────────────────
Write-Host ''
Write-Host '  Outils du projet' -ForegroundColor White
Test-Outil -Nom 'git'         -Commande 'git'         -Installation 'winget install Git.Git' | Out-Null
Test-Outil -Nom 'cargo-audit' -Commande 'cargo-audit' -Installation 'cargo install cargo-audit' -Optionnel | Out-Null
Test-Outil -Nom 'cargo-deny'  -Commande 'cargo-deny'  -Installation 'cargo install cargo-deny'  -Optionnel | Out-Null

# ─── Le shell lui-même ─────────────────────────────────────────────────────
Write-Host ''
Write-Host '  Shell' -ForegroundColor White
Write-Host ("  [ok]   Windows PowerShell     {0} — suffit pour tous les scripts du dépôt" -f $PSVersionTable.PSVersion) -ForegroundColor Green
Test-Outil -Nom 'PowerShell 7' -Commande 'pwsh' `
    -Installation 'winget install Microsoft.PowerShell' -Optionnel | Out-Null
Write-Host '         Recommandé pour le confort interactif, jamais requis par les scripts.' -ForegroundColor DarkGray

# ─── Contexte de la machine ────────────────────────────────────────────────
Write-Host ''
Write-Host '  Contexte de la machine' -ForegroundColor White

# `Get-WindowsOptionalFeature -Online` exige l'élévation. Sans elle, l'appel
# échoue et l'ancien code concluait « Hyper-V absent » — un faux négatif sur le
# cas normal, puisque ce script est censé tourner sans privilège. On distingue
# donc « désactivé » de « indéterminable ».
$eleve = ([Security.Principal.WindowsPrincipal] `
    [Security.Principal.WindowsIdentity]::GetCurrent()
).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

$hv = $null
if ($eleve) {
    $hv = Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V-All -ErrorAction SilentlyContinue
}

# Sans élévation, la présence du module de gestion est un bon indicateur de
# substitution : il n'est livré qu'avec le rôle Hyper-V. À ne pas confondre avec
# `HypervisorPresent`, qui vaut True dès que WSL2 ou Docker Desktop tourne — la
# plateforme de virtualisation suffit à ceux-là, pas à créer une VM de labo.
$outilsHv = [bool](Get-Command Get-VM -ErrorAction SilentlyContinue)

if (($hv -and $hv.State -eq 'Enabled') -or (-not $eleve -and $outilsHv)) {
    Write-Host '  [ok]   Hyper-V                 activé — la VM de labo est possible' -ForegroundColor Green
} elseif (-not $eleve) {
    Write-Host '  [opt]  Hyper-V                 ' -NoNewline -ForegroundColor Yellow
    Write-Host 'outils de gestion absents — requis seulement à partir de la Phase 2' -ForegroundColor DarkGray
} else {
    Write-Host '  [opt]  Hyper-V                 ' -NoNewline -ForegroundColor Yellow
    Write-Host 'requis à partir de la Phase 2 (voir docs/06-VM-DE-LABO.md)' -ForegroundColor DarkGray
}

if (Get-Command wsl -ErrorAction SilentlyContinue) {
    # `wsl.exe` écrit en UTF-16LE. Windows PowerShell 5.1 lit la sortie d'un
    # exécutable natif avec l'encodage console courant : sans le forcer, chaque
    # caractère est suivi d'un octet nul et « Debian » s'affiche « D e b i a n ».
    # On restaure l'encodage ensuite pour ne rien laisser derrière soi.
    $encodagePrecedent = [Console]::OutputEncoding
    try {
        [Console]::OutputEncoding = [System.Text.Encoding]::Unicode
        $distros = (wsl --list --quiet 2>$null |
            Where-Object { $_ -and $_.Trim() }) -join ', '
    } finally {
        [Console]::OutputEncoding = $encodagePrecedent
    }
    if (-not $distros) { $distros = 'aucune distribution installée' }
    Write-Host ('  [ok]   WSL                     ' + $distros) -ForegroundColor Green
} else {
    Write-Host '  [opt]  WSL                     ' -NoNewline -ForegroundColor Yellow
    Write-Host 'nécessaire pour tester ks-agent' -ForegroundColor DarkGray
}

# Chemins longs : Rust + Windows + arborescence profonde = échecs obscurs.
$lp = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -ErrorAction SilentlyContinue
if ($lp.LongPathsEnabled -eq 1) {
    Write-Host '  [ok]   chemins longs           activés' -ForegroundColor Green
} else {
    Write-Host '  [opt]  chemins longs           ' -NoNewline -ForegroundColor Yellow
    Write-Host 'voir docs/05-ENVIRONNEMENT-DE-DEV.md § Pièges' -ForegroundColor DarkGray
}

# ─── Verdict ───────────────────────────────────────────────────────────────
Write-Host ''
if ($script:Manquants.Count -eq 0) {
    Write-Host '  Environnement complet.' -ForegroundColor Green
    Write-Host ''
    Write-Host '  Prochaine étape — la Phase 0 est en LECTURE SEULE, aucun risque :' -ForegroundColor White
    Write-Host '    cargo test --workspace' -ForegroundColor Cyan
    Write-Host '    cargo run -p ks-cli -- scan' -ForegroundColor Cyan
    Write-Host '    cargo run -p ks-cli -- status' -ForegroundColor Cyan
} else {
    Write-Host ('  ' + $script:Manquants.Count + ' élément(s) requis manquant(s) :') -ForegroundColor Red
    $script:Manquants | ForEach-Object { Write-Host ('    · ' + $_) -ForegroundColor Red }
    Write-Host ''
    Write-Host '  Les commandes d''installation sont indiquées ci-dessus.' -ForegroundColor DarkGray
    Write-Host '  Détail complet : docs/05-ENVIRONNEMENT-DE-DEV.md' -ForegroundColor DarkGray
}
Write-Host ''
