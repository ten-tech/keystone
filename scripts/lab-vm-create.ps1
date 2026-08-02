#Requires -Version 5.1
#Requires -RunAsAdministrator
# PowerShell 5.1 et non 7 : le module Hyper-V est un module Windows PowerShell.
# Sous PowerShell 7 il se charge via la couche de compatibilité, qui renvoie des
# objets désérialisés — un objet de point de contrôle repassé à Restore-VMSnapshot
# dépend alors d'un comportement indirect. Natif en 5.1, c'est prévisible.
<#
.SYNOPSIS
    Crée la VM de labo Keystone, avec les trois réglages qu'on oublie toujours.

.DESCRIPTION
    À partir de la Phase 2, Keystone écrit dans le système. Il se teste donc ici,
    et nulle part ailleurs — surtout pas sur la machine dont tu as besoin demain
    matin.

    Ce script applique les trois pièges documentés dans docs/06-VM-DE-LABO.md :

      1. VIRTUALISATION IMBRIQUÉE — sans elle, WSL2 ne démarre pas dans la VM,
         et la moitié du produit devient intestable.
      2. vTPM — sans lui : pas de TPM, pas de BitLocker, pas d'attestation PCR.
         Les collecteurs du domaine D5 liraient du vide et tu croirais à un bug
         de ton code.
      3. MÉMOIRE STATIQUE — Hyper-V imbriqué l'exige.

    Le script s'arrête AVANT l'installation de Windows, qui reste manuelle.
    Automatiser l'installation d'un OS pour gagner vingt minutes une fois par an
    n'en vaut pas la complexité.

.PARAMETER Name
    Nom de la VM. Défaut : ks-lab

.PARAMETER IsoPath
    Chemin de l'ISO Windows 11.

.PARAMETER MemoryGB
    Mémoire STATIQUE en Go. Défaut : 12. Sous 8, WSL2 dans la VM est inutilisable.

.PARAMETER DiskGB
    Taille du disque en Go. Défaut : 100.

.PARAMETER CpuCount
    Nombre de vCPU. Défaut : 6.

.EXAMPLE
    .\scripts\lab-vm-create.ps1 -IsoPath D:\iso\Win11_26H1.iso
#>

[CmdletBinding()]
param(
    [string] $Name = 'ks-lab',
    [Parameter(Mandatory)] [string] $IsoPath,
    [int] $MemoryGB = 12,
    [int] $DiskGB = 100,
    [int] $CpuCount = 6,
    [string] $VmPath = 'D:\vms',
    [string] $SwitchName = 'Default Switch'
)

$ErrorActionPreference = 'Stop'

function Etape { param([string] $m) Write-Host ('  → ' + $m) -ForegroundColor Cyan }
function Ok    { param([string] $m) Write-Host ('  ✓ ' + $m) -ForegroundColor Green }

Write-Host ''
Write-Host '  KEYSTONE — création de la VM de labo' -ForegroundColor Cyan
Write-Host ''

# ─── Contrôles préalables ──────────────────────────────────────────────────
if (-not (Test-Path $IsoPath)) { throw "ISO introuvable : $IsoPath" }

$hv = Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V-All
if ($hv.State -ne 'Enabled') {
    throw 'Hyper-V n''est pas activé. Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All'
}

if (Get-VM -Name $Name -ErrorAction SilentlyContinue) {
    throw "La VM « $Name » existe déjà. Pour repartir de zéro : Remove-VM -Name $Name -Force"
}

if ($MemoryGB -lt 8) {
    Write-Warning 'Sous 8 Go, WSL2 dans une VM imbriquée est inutilisable. 12 Go recommandés.'
}

New-Item -ItemType Directory -Force -Path $VmPath | Out-Null
$vhd = Join-Path $VmPath "$Name.vhdx"

# ─── Création ──────────────────────────────────────────────────────────────
Etape 'Création de la VM (génération 2, requise pour le vTPM)'
New-VM -Name $Name `
       -Generation 2 `
       -MemoryStartupBytes ($MemoryGB * 1GB) `
       -NewVHDPath $vhd `
       -NewVHDSizeBytes ($DiskGB * 1GB) `
       -SwitchName $SwitchName `
       -Path $VmPath | Out-Null
Ok "VM « $Name » créée — $MemoryGB Go, $DiskGB Go, $CpuCount vCPU"

Etape 'PIÈGE 1 — virtualisation imbriquée (sans elle, pas de WSL2 dans la VM)'
Set-VMProcessor -VMName $Name -Count $CpuCount -ExposeVirtualizationExtensions $true
Set-VMNetworkAdapter -VMName $Name -MacAddressSpoofing On
Ok 'nested virt activé, usurpation MAC autorisée'

Etape 'PIÈGE 3 — mémoire statique (Hyper-V imbriqué l''exige)'
Set-VMMemory -VMName $Name -DynamicMemoryEnabled $false
Ok 'mémoire dynamique désactivée'

Etape 'PIÈGE 2 — vTPM (sans lui : pas de TPM, pas de BitLocker, pas de PCR)'
Set-VMKeyProtector -VMName $Name -NewLocalKeyProtector
Enable-VMTPM -VMName $Name
Ok 'vTPM activé'

Etape 'Secure Boot et ordre de démarrage'
Set-VMFirmware -VMName $Name -EnableSecureBoot On -SecureBootTemplate 'MicrosoftWindows'
Add-VMDvdDrive -VMName $Name -Path $IsoPath
$dvd = Get-VMDvdDrive -VMName $Name
Set-VMFirmware -VMName $Name -FirstBootDevice $dvd
Ok 'Secure Boot activé, démarrage sur l''ISO'

Etape 'Points de contrôle en mode production (cohérents applicativement)'
Set-VM -Name $Name -CheckpointType Production -AutomaticCheckpointsEnabled $false
Ok 'points de contrôle automatiques désactivés — on les prend nous-mêmes'

# ─── Suite ─────────────────────────────────────────────────────────────────
Write-Host ''
Write-Host '  VM prête à recevoir Windows.' -ForegroundColor Green
Write-Host ''
Write-Host '  À FAIRE MAINTENANT' -ForegroundColor White
Write-Host ''
Write-Host ('    1.  Démarrer et installer Windows 11 :  Start-VM -Name ' + $Name) -ForegroundColor Cyan
Write-Host ('                                            vmconnect localhost ' + $Name) -ForegroundColor Cyan
Write-Host ''
Write-Host '    2.  DANS la VM, reproduire un poste d''ingénieur crédible :' -ForegroundColor White
Write-Host '          wsl --install --no-launch' -ForegroundColor Cyan
Write-Host '          Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All -NoRestart' -ForegroundColor Cyan
Write-Host ''
Write-Host '    3.  DANS la VM, autoriser la signature de test :' -ForegroundColor White
Write-Host '          bcdedit /set testsigning on' -ForegroundColor Cyan
Write-Host '        ⚠  DANS LA VM UNIQUEMENT. Jamais sur l''hôte.' -ForegroundColor Yellow
Write-Host ''
Write-Host '    4.  Redémarrer, éteindre, puis prendre LE point de contrôle de référence :' -ForegroundColor White
Write-Host ('          Checkpoint-VM -VMName ' + $Name + ' -SnapshotName clean') -ForegroundColor Cyan
Write-Host ''
Write-Host '        C''est ce point de contrôle qui rend la boucle rapide : un cycle' -ForegroundColor DarkGray
Write-Host '        « applique → observe → annule » prendra 20 secondes.' -ForegroundColor DarkGray
Write-Host '        Ne jamais le supprimer ni l''écraser.' -ForegroundColor DarkGray
Write-Host ''
Write-Host '  Détail complet : docs/06-VM-DE-LABO.md' -ForegroundColor DarkGray
Write-Host ''
