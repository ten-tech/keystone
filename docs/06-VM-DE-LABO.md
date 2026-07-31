# 06 — La VM de labo

> **À lire avant la Phase 2.** Tant que le code est en lecture seule, il tourne sans
> risque sur l'hôte. Dès qu'il écrit, il tourne ici et nulle part ailleurs.

## Pourquoi

Keystone modifie le système d'exploitation : services, registre, politiques,
Defender, BitLocker, WSL, Hyper-V. Le tester sur sa propre machine, c'est la casser —
au mieux une fois, au pire lentement et sans s'en rendre compte.

Il y a aussi une raison qui n'est pas défensive : **la VM jetable est le seul moyen
d'itérer vite.** Avec un point de contrôle nommé `clean`, un cycle
« applique → observe → annule » prend 20 secondes. Sur l'hôte, il prendrait une
soirée et laisserait des traces.

Et accessoirement, ça satisfait l'exigence NF-07 du produit : la matrice de VM
éphémères en intégration continue.

## Les trois pièges

Ce sont trois heures perdues si on ne les connaît pas, et ils sont tous les trois
propres à ce projet.

### 1. Virtualisation imbriquée

Keystone gère WSL2 et Hyper-V. Sans virtualisation imbriquée, **WSL2 ne démarre pas
dans la VM** — et la moitié du produit devient intestable.

```powershell
Set-VMProcessor       -VMName ks-lab -ExposeVirtualizationExtensions $true
Set-VMNetworkAdapter  -VMName ks-lab -MacAddressSpoofing On
Set-VMMemory          -VMName ks-lab -DynamicMemoryEnabled $false
```

La mémoire dynamique doit être désactivée : Hyper-V imbriqué l'exige.

### 2. vTPM

Sans TPM virtuel : pas de TPM, pas de BitLocker, pas d'attestation PCR, pas de
Credential Guard. Les collecteurs de posture du domaine D5 — le cœur de la valeur
sécurité du produit — liraient du vide et tu croirais à un bug de ton code.

```powershell
Set-VMKeyProtector -VMName ks-lab -NewLocalKeyProtector
Enable-VMTPM       -VMName ks-lab
```

C'est aussi un prérequis de Windows 11 pour l'installation, donc de toute façon
incontournable.

### 3. Signature de test

`ks-broker` refuse les modules non signés (exigence SEC-07). En développement, on
signe avec un certificat auto-généré, ce qui exige d'autoriser la signature de test.

**Dans la VM uniquement. Jamais sur l'hôte.**

```powershell
# À exécuter DANS la VM, en administrateur
bcdedit /set testsigning on
# puis redémarrer
```

Générer le certificat de développement, également dans la VM :

```powershell
$cert = New-SelfSignedCertificate -Type CodeSigningCert `
    -Subject "CN=Keystone Dev" -CertStoreLocation Cert:\CurrentUser\My
Export-Certificate -Cert $cert -FilePath C:\keystone-dev.cer
Import-Certificate -FilePath C:\keystone-dev.cer -CertStoreLocation Cert:\LocalMachine\Root
Import-Certificate -FilePath C:\keystone-dev.cer -CertStoreLocation Cert:\LocalMachine\TrustedPublisher
```

> Le certificat de développement ne quitte **jamais** la VM, et n'est **jamais**
> versionné : `.gitignore` bloque déjà `*.pfx`, `*.p12` et `certs/dev-*`. Un dépôt
> qui contient une clé de signature est un dépôt qui contient une porte d'entrée.

## Création

```powershell
.\scripts\lab-vm-create.ps1 -Name ks-lab -IsoPath D:\iso\Win11_26H1.iso
```

Le script applique les trois points ci-dessus, crée la VM, et s'arrête avant
l'installation de Windows — qui reste manuelle et le restera : automatiser
l'installation d'un OS pour gagner vingt minutes une fois par an n'en vaut pas la
complexité.

## Après l'installation de Windows, dans la VM

```powershell
# 1. Reproduire un poste d'ingénieur crédible : sans WSL ni Hyper-V,
#    la VM ne ressemble pas à la cible du produit.
wsl --install --no-launch
Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All -NoRestart

# 2. Autoriser la signature de test (voir piège 3)
bcdedit /set testsigning on

# 3. Redémarrer, puis prendre LE point de contrôle de référence
```

Depuis l'hôte, une fois la VM prête et éteinte :

```powershell
Checkpoint-VM -VMName ks-lab -SnapshotName clean
```

**C'est ce point de contrôle qui rend la boucle rapide.** Ne jamais le supprimer, ne
jamais l'écraser : quand la VM a besoin d'évoluer, on crée `clean-2` et on garde
l'ancien.

## La boucle de développement

```powershell
# 1. Construire sur l'hôte
cargo build --release

# 2. Envoyer dans la VM (session PowerShell Direct — pas besoin de réseau)
$s = New-PSSession -VMName ks-lab -Credential (Get-Credential)
Copy-Item .\target\release\ks-broker.exe -Destination C:\ks\ -ToSession $s
Copy-Item .\target\release\ks.exe        -Destination C:\ks\ -ToSession $s

# 3. Exécuter et observer
# Sans `--apply`, `converge` simule : c'est le comportement par défaut (P2).
Invoke-Command -Session $s -ScriptBlock { C:\ks\ks.exe converge }

# 4. Remettre à zéro
.\scripts\lab-vm-reset.ps1
```

`PowerShell Direct` (`-VMName`) fonctionne **sans réseau dans l'invité**. C'est le
même raisonnement que le choix d'Hyper-V Socket pour l'agent : on veut pouvoir
atteindre la VM au moment précis où son réseau est cassé, c'est-à-dire quand on en a
le plus besoin.

## Dimensionnement

| Ressource | Minimum | Confortable |
|---|---|---|
| vCPU | 4 | 8 |
| Mémoire (statique, nested l'exige) | 8 Go | 16 Go |
| Disque | 80 Go | 120 Go |

La virtualisation imbriquée et WSL2 dans la VM sont gourmands. Sous 8 Go, WSL2 dans
la VM devient inutilisable.

## Ce qu'il faudra ajouter plus tard

| Quand | Quoi |
|---|---|
| Phase 3 | une seconde VM avec **Intune enrôlé**, pour tester la coexistence MDM (D12) — les conflits de politique ne se simulent pas |
| Phase 3 | un puits de journal externe (un simple conteneur avec un stockage en écriture seule) pour éprouver SEC-04 et SEC-05 |
| Phase 4 | une VM par édition de Windows visée, pour la matrice de NF-07 |
| Phase 6 | l'automatisation complète de la création du labo, pour que la CI la fabrique elle-même |

## Rappel

Le seul endroit où Keystone ne doit **jamais** tourner en développement, c'est la
machine dont tu as besoin pour travailler demain matin.
