#Requires -Version 7.0
#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Remet la VM de labo à son point de contrôle de référence.

.DESCRIPTION
    C'est ce qui rend la boucle de développement viable : un cycle
    « applique → observe → annule » en une vingtaine de secondes.

    Note de conception : ce script est le seul du projet à ne PAS simuler par
    défaut, parce que son effet EST l'annulation. Il montre quand même ce qu'il
    va faire avant de le faire, et demande confirmation. -Force la saute.

.PARAMETER Name
    Nom de la VM. Défaut : ks-lab

.PARAMETER Checkpoint
    Point de contrôle de référence. Défaut : clean

.PARAMETER Start
    Démarrer la VM après restauration.

.PARAMETER Force
    Ne pas demander confirmation.

.EXAMPLE
    .\scripts\lab-vm-reset.ps1 -Start -Force
#>

[CmdletBinding()]
param(
    [string] $Name = 'ks-lab',
    [string] $Checkpoint = 'clean',
    [switch] $Start,
    [switch] $Force
)

$ErrorActionPreference = 'Stop'

$vm = Get-VM -Name $Name -ErrorAction SilentlyContinue
if (-not $vm) {
    throw "VM « $Name » introuvable. La créer : .\scripts\lab-vm-create.ps1 -IsoPath <iso>"
}

$snap = Get-VMSnapshot -VMName $Name -Name $Checkpoint -ErrorAction SilentlyContinue
if (-not $snap) {
    $dispo = (Get-VMSnapshot -VMName $Name | Select-Object -ExpandProperty Name) -join ', '
    throw "Point de contrôle « $Checkpoint » introuvable. Disponibles : $dispo"
}

Write-Host ''
Write-Host ('  VM             ' + $Name) -ForegroundColor White
Write-Host ('  État actuel    ' + $vm.State) -ForegroundColor DarkGray
Write-Host ('  Restaurer vers ' + $Checkpoint + '  (' + $snap.CreationTime.ToString('yyyy-MM-dd HH:mm') + ')') -ForegroundColor Cyan
Write-Host ''
Write-Host '  Tout ce qui a été fait dans la VM depuis ce point sera perdu.' -ForegroundColor Yellow
Write-Host ''

if (-not $Force) {
    $r = Read-Host '  Continuer ? [o/N]'
    if ($r -notmatch '^[oOyY]') {
        Write-Host '  Annulé.' -ForegroundColor DarkGray
        return
    }
}

if ($vm.State -ne 'Off') {
    Write-Host '  → Arrêt' -ForegroundColor Cyan
    Stop-VM -Name $Name -TurnOff -Force
}

Write-Host '  → Restauration' -ForegroundColor Cyan
Restore-VMSnapshot -VMSnapshot $snap -Confirm:$false

if ($Start) {
    Write-Host '  → Démarrage' -ForegroundColor Cyan
    Start-VM -Name $Name
}

Write-Host ''
Write-Host ('  ✓ Revenu à « ' + $Checkpoint + ' »') -ForegroundColor Green
Write-Host ''
Write-Host '  Pour envoyer les binaires — PowerShell Direct, sans réseau dans l''invité :' -ForegroundColor DarkGray
Write-Host ('    $s = New-PSSession -VMName ' + $Name + ' -Credential (Get-Credential)') -ForegroundColor DarkGray
Write-Host '    Copy-Item .\target\release\ks.exe -Destination C:\ks\ -ToSession $s' -ForegroundColor DarkGray
Write-Host ''
