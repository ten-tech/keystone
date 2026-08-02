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

# ─── Rendu ─────────────────────────────────────────────────────────────────
#
# Une seule fonction pose une ligne, et toutes les colonnes en découlent.
# Auparavant chaque appel refaisait l'alignement à la main : « [ok] » était
# précédé de deux espaces, « [opt] » d'aucun, et la colonne des valeurs variait
# d'un bloc à l'autre. Un alignement recopié dérive toujours.

# La console sait-elle rendre les glyphes fins ? On ne le suppose pas : on
# éprouve l'encodage de sortie par un aller-retour. Une console en page de codes
# héritée remplacerait « ✓ » par « ? » — mieux vaut un jeu ASCII assumé qu'un
# rendu abîmé.
function Test-GlyphesFins {
    try {
        $e = [Console]::OutputEncoding
        return $e.GetString($e.GetBytes('✓·─')) -eq '✓·─'
    } catch { return $false }
}
$script:Fin = Test-GlyphesFins

$script:Glyphe = if ($script:Fin) {
    @{ ok = '✓'; opt = '·'; abs = '✗'; filet = '─'; sep = ' · '; puce = '·' }
} else {
    @{ ok = '+'; opt = '-'; abs = 'x'; filet = '-'; sep = ', ';  puce = '-' }
}

$script:Marge     = 2                    # marge de page
$script:Retrait   = 4                    # les items sont en retrait sous leur section
$script:ColEtat   = 3                    # glyphe + deux espaces
$script:ColNom    = 22
$script:Largeur   = 78
$script:ColValeur = $script:Retrait + $script:ColEtat + $script:ColNom

# Un intitulé de section suivi d'un filet qui court jusqu'au bord. C'est la
# transposition en terminal du « filet de 2 px » du brief de design : on cadre
# sans encadrer, on sépare sans bannière.
function Write-Section {
    # Titre vide accepté : le même filet sert alors de séparation avant le verdict.
    param([string] $Titre = '')
    Write-Host ''
    if ($Titre) {
        $reste = [Math]::Max(0, $script:Largeur - $script:Marge - $Titre.Length - 1)
        Write-Host ((' ' * $script:Marge) + $Titre + ' ') -NoNewline -ForegroundColor Gray
        Write-Host ($script:Glyphe.filet * $reste) -ForegroundColor DarkGray
    } else {
        $reste = [Math]::Max(0, $script:Largeur - $script:Marge)
        Write-Host ((' ' * $script:Marge) + ($script:Glyphe.filet * $reste)) -ForegroundColor DarkGray
    }
}

function Write-Ligne {
    param(
        [Parameter(Mandatory)] [ValidateSet('ok', 'opt', 'abs')] [string] $Etat,
        [Parameter(Mandatory)] [string] $Nom,
        [string] $Valeur = ''
    )
    switch ($Etat) {
        'ok'  { $couleur = 'Green'  }
        'opt' { $couleur = 'DarkYellow' }
        'abs' { $couleur = 'Red'    }
    }
    Write-Host ((' ' * $script:Retrait) + $script:Glyphe[$Etat].PadRight($script:ColEtat)) -NoNewline -ForegroundColor $couleur
    Write-Host $Nom.PadRight($script:ColNom) -NoNewline -ForegroundColor Gray
    Write-Host $Valeur -ForegroundColor DarkGray
}

# Ligne de continuation, alignée sur la colonne des valeurs.
function Write-Note {
    param([Parameter(Mandatory)] [string] $Texte)
    Write-Host ((' ' * $script:ColValeur) + $Texte) -ForegroundColor DarkGray
}

# « rustc 1.97.1 (…) » → « 1.97.1 (…) », « git version 2.55 » → « 2.55 ».
# Le nom occupe déjà sa colonne : le répéter dans la valeur est du bruit.
function Format-Version {
    param([string] $Brut, [string] $Commande, [string] $Nom)
    $v = (($Brut -replace '\s+', ' ')).Trim()
    foreach ($prefixe in @($Commande, ($Nom -split ' ')[0], 'version')) {
        if ($prefixe -and $v.StartsWith($prefixe + ' ', 'OrdinalIgnoreCase')) {
            $v = $v.Substring($prefixe.Length + 1).Trim()
        }
    }
    return $v
}

function Test-Outil {
    param(
        [Parameter(Mandatory)] [string] $Nom,
        [Parameter(Mandatory)] [string] $Commande,
        [Parameter(Mandatory)] [string] $Installation,
        [string] $VersionArg = '--version',
        [switch] $Optionnel
    )

    if (Get-Command $Commande -ErrorAction SilentlyContinue) {
        $brut = & $Commande $VersionArg 2>&1 | Select-Object -First 1
        Write-Ligne -Etat 'ok' -Nom $Nom -Valeur (Format-Version -Brut $brut -Commande $Commande -Nom $Nom)
        return $true
    }

    Write-Ligne -Etat $(if ($Optionnel) { 'opt' } else { 'abs' }) -Nom $Nom -Valeur $Installation
    if (-not $Optionnel) { $script:Manquants += $Nom }
    return $false
}

Write-Host ''
Write-Host ((' ' * $script:Marge) + 'KEYSTONE') -NoNewline -ForegroundColor Cyan
Write-Host ('  vérification de l''environnement de développement') -ForegroundColor DarkGray
Write-Host ((' ' * $script:Marge) + 'Ce script ne modifie rien : il constate, il rapporte.') -ForegroundColor DarkGray

# ─── Chaîne Rust ───────────────────────────────────────────────────────────
Write-Section 'Chaîne Rust'
Test-Outil -Nom 'rustc'   -Commande 'rustc'   -Installation 'winget install Rustlang.Rustup' | Out-Null
Test-Outil -Nom 'cargo'   -Commande 'cargo'   -Installation 'winget install Rustlang.Rustup' | Out-Null
Test-Outil -Nom 'rustup'  -Commande 'rustup'  -Installation 'winget install Rustlang.Rustup' | Out-Null

if (Get-Command rustup -ErrorAction SilentlyContinue) {
    # Une ligne par famille plutôt qu'une par élément : quatre lignes pour dire
    # « tout est là » est du remplissage. Ce qui manque, en revanche, mérite sa
    # propre ligne et sa commande d'installation.
    $installees = rustup target list --installed
    $attendues  = @('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-musl')
    $presentes  = @($attendues | Where-Object { $installees -contains $_ })
    $absentes   = @($attendues | Where-Object { $installees -notcontains $_ })
    if ($presentes) {
        Write-Ligne -Etat 'ok' -Nom 'cibles' -Valeur ($presentes -join $script:Glyphe.sep)
    }
    foreach ($c in $absentes) {
        Write-Ligne -Etat 'abs' -Nom 'cible' -Valeur ('rustup target add ' + $c)
        $script:Manquants += ('cible ' + $c)
    }

    # `$composants -notmatch $c` sur un TABLEAU renvoie les éléments qui ne
    # correspondent pas — donc une liste non vide, donc vraie. Les deux filtres
    # passaient ensemble, et chaque composant s'affichait présent ET absent.
    # On teste donc la présence d'au moins un élément, pas la véracité de -match.
    $composants = rustup component list --installed
    $attendus   = @('rustfmt', 'clippy')
    $ok = @($attendus | Where-Object { $n = $_; @($composants | Where-Object { $_ -like "$n*" }).Count -gt 0 })
    $ko = @($attendus | Where-Object { $n = $_; @($composants | Where-Object { $_ -like "$n*" }).Count -eq 0 })
    if ($ok) {
        Write-Ligne -Etat 'ok' -Nom 'composants' -Valeur ($ok -join $script:Glyphe.sep)
    }
    foreach ($c in $ko) {
        Write-Ligne -Etat 'abs' -Nom 'composant' -Valeur ('rustup component add ' + $c)
        $script:Manquants += ('composant ' + $c)
    }
}

# ─── Linker MSVC ───────────────────────────────────────────────────────────
# La cible du produit est -msvc, pas -gnu : le broker tape dans WMI, DPAPI-NG,
# WFP et l'API TPM. Un binaire -gnu n'est pas le binaire qu'on livre.
Write-Section 'Outils de compilation Microsoft'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path $vswhere) {
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion 2>$null
    if ($vs) {
        Write-Ligne -Etat 'ok' -Nom 'outils C++ MSVC' -Valeur $vs
    } else {
        Write-Ligne -Etat 'abs' -Nom 'outils C++ MSVC' -Valeur 'installeur VS → « Développement Desktop en C++ » + SDK Windows'
        $script:Manquants += 'outils C++ MSVC'
    }
} else {
    Write-Ligne -Etat 'abs' -Nom 'Visual Studio Build Tools' -Valeur 'winget install Microsoft.VisualStudio.2022.BuildTools'
    $script:Manquants += 'Visual Studio Build Tools'
}

# ─── Cross-compilation de l'agent Linux ────────────────────────────────────
Write-Section 'Cross-compilation de l''agent Linux'
$zb = Test-Outil -Nom 'cargo-zigbuild' -Commande 'cargo-zigbuild' -Installation 'cargo install cargo-zigbuild' -Optionnel
$zig = Test-Outil -Nom 'zig' -Commande 'zig' -Installation 'winget install zig.zig' -Optionnel -VersionArg 'version'
if (-not ($zb -and $zig)) {
    Write-Note 'Alternative si Docker est déjà installé : cargo install cross'
}

# ─── Le reste ──────────────────────────────────────────────────────────────
Write-Section 'Outils du projet'
Test-Outil -Nom 'git'         -Commande 'git'         -Installation 'winget install Git.Git' | Out-Null
Test-Outil -Nom 'cargo-audit' -Commande 'cargo-audit' -Installation 'cargo install cargo-audit' -Optionnel | Out-Null
Test-Outil -Nom 'cargo-deny'  -Commande 'cargo-deny'  -Installation 'cargo install cargo-deny'  -Optionnel | Out-Null

# ─── Le shell lui-même ─────────────────────────────────────────────────────
Write-Section 'Shell'
# `PSEdition` vaut « Desktop » pour Windows PowerShell 5.1 et « Core » pour
# PowerShell 7 : les nommer pareil serait faux, ce sont deux produits distincts.
$nomShell = if ($PSVersionTable.PSEdition -eq 'Core') { 'PowerShell' } else { 'Windows PowerShell' }
Write-Ligne -Etat 'ok' -Nom $nomShell -Valeur ("{0} — suffit pour tous les scripts du dépôt" -f $PSVersionTable.PSVersion)

if ($PSVersionTable.PSEdition -ne 'Core') {
    Test-Outil -Nom 'PowerShell 7' -Commande 'pwsh' `
        -Installation 'winget install Microsoft.PowerShell' -Optionnel | Out-Null
    Write-Note 'Recommandé pour le confort interactif, jamais requis par les scripts.'
}

# ─── Contexte de la machine ────────────────────────────────────────────────
Write-Section 'Contexte de la machine'

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
    Write-Ligne -Etat 'ok' -Nom 'Hyper-V' -Valeur 'activé — la VM de labo est possible'
} elseif (-not $eleve) {
    Write-Ligne -Etat 'opt' -Nom 'Hyper-V' -Valeur 'outils de gestion absents — requis à partir de la Phase 2'
} else {
    Write-Ligne -Etat 'opt' -Nom 'Hyper-V' -Valeur 'requis à partir de la Phase 2 (voir docs/06-VM-DE-LABO.md)'
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
    Write-Ligne -Etat 'ok' -Nom 'WSL' -Valeur $distros
} else {
    Write-Ligne -Etat 'opt' -Nom 'WSL' -Valeur 'nécessaire pour tester ks-agent'
}

# Chemins longs : Rust + Windows + arborescence profonde = échecs obscurs.
$lp = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -ErrorAction SilentlyContinue
if ($lp.LongPathsEnabled -eq 1) {
    Write-Ligne -Etat 'ok' -Nom 'chemins longs' -Valeur 'activés'
} else {
    Write-Ligne -Etat 'opt' -Nom 'chemins longs' -Valeur 'voir docs/05-ENVIRONNEMENT-DE-DEV.md § Pièges'
}

# ─── Verdict ───────────────────────────────────────────────────────────────
# « Le calme est la fonctionnalité » : quand tout va bien, on le dit en une
# ligne et on s'arrête. Le détail ne s'impose que lorsqu'il manque quelque chose.
# `$decalage` et non `$marge` : PowerShell est INSENSIBLE À LA CASSE sur les noms
# de variables, donc au niveau du script `$marge` EST `$script:Marge`. Écrire
# `$marge = ' ' * $script:Marge` remplaçait le nombre 2 par la chaîne « ␣␣ », et
# tout calcul de largeur ultérieur tombait à zéro — le filet final partait alors
# de la colonne 0. Même famille de piège que la réassignation de `$args`.
$decalage = ' ' * $script:Marge
Write-Section ''

if ($script:Manquants.Count -eq 0) {
    Write-Host ($decalage + $script:Glyphe.ok + '  Environnement complet.') -ForegroundColor Green
    Write-Host ''
    Write-Host ($decalage + 'La Phase 0 est en LECTURE SEULE — rien n''est écrit sur la machine.') -ForegroundColor DarkGray
    foreach ($cmd in @('cargo test --workspace',
                       'cargo run -p ks-cli -- scan',
                       'cargo run -p ks-cli -- status')) {
        Write-Host ($decalage + '  ') -NoNewline
        Write-Host $script:Glyphe.puce -NoNewline -ForegroundColor DarkGray
        Write-Host (' ' + $cmd) -ForegroundColor Cyan
    }
} else {
    $n   = $script:Manquants.Count
    $mot = if ($n -gt 1) { 'éléments requis manquants' } else { 'élément requis manquant' }
    Write-Host ($decalage + $script:Glyphe.abs + "  $n $mot") -ForegroundColor Red
    Write-Host ''
    foreach ($m in $script:Manquants) {
        Write-Host ($decalage + '  ') -NoNewline
        Write-Host $script:Glyphe.puce -NoNewline -ForegroundColor DarkGray
        Write-Host (' ' + $m) -ForegroundColor Red
    }
    Write-Host ''
    Write-Host ($decalage + 'Les commandes d''installation sont indiquées ci-dessus.') -ForegroundColor DarkGray
    Write-Host ($decalage + 'Détail complet : docs/05-ENVIRONNEMENT-DE-DEV.md') -ForegroundColor DarkGray
}
Write-Host ''
