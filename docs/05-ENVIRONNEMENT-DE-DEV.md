# 05 — Environnement de développement

## La question courte : je code où ?

**Sur Windows, nativement.** `D:\src\keystone` (ou l'emplacement de ton choix, en
NTFS local), VS Code lancé côté Windows, toolchain Rust `x86_64-pc-windows-msvc`.

Pas dans WSL cette fois. Et pas sur la machine de production dès que le code écrit.

## Trois emplacements, trois rôles

| Quoi | Où on l'écrit et le compile | Où on l'exécute |
|---|---|---|
| `ks-broker`, `ks-cli`, `ks-collectors`, UI Tauri | **Windows hôte, nativement** | Phase 0 : l'hôte (lecture seule) · Phase 2+ : **VM de labo uniquement** |
| `ks-agent-linux` | Windows, cross-compilé vers `x86_64-unknown-linux-musl` | une distro WSL2 |
| `ks-core` | n'importe où — il est portable et pur | tests unitaires partout, y compris en CI Linux |

## Pourquoi pas développer la partie Windows depuis WSL

C'est le réflexe habituel, et pour la plupart des projets c'est le bon. Ici, trois
raisons concrètes de ne pas le faire :

**1. La cible est MSVC, pas MinGW.** Le broker tape dans WMI, DPAPI-NG, WFP, l'API
TPM et le gestionnaire de services. Depuis WSL on ne peut viser que
`x86_64-pc-windows-gnu`. Ce n'est pas « la même chose en moins bien » : c'est une
autre ABI, d'autres bibliothèques d'import, d'autres comportements sur les
structures Win32. Tu debuggerais un binaire qui n'est pas celui que tu livres.

**2. Le débogage est côté Windows.** Attacher un débogueur à un service, lire les
crashs dans WER, décoder un code d'arrêt, tracer ETW, inspecter l'Observateur
d'événements : rien de tout ça ne traverse la frontière WSL.

**3. `/mnt/c` est lent.** La traduction 9P/DrvFs coûte cher sur les milliers de
petits fichiers d'un workspace Rust. Un `cargo build` sur un projet posé là est
plusieurs fois plus lent qu'en NTFS natif.

## Pourquoi la Phase 0 peut tourner sur ta machine

Parce qu'elle **n'écrit rien**. C'est une propriété vérifiée par le code, pas une
promesse : voir la règle en tête de `crates/ks-collectors/src/lib.rs` et le test
`aucun_item_collecte_nest_declare_desire`.

C'est un des intérêts de la séquence choisie : tu as plusieurs semaines de
développement utile — inventaire, réconciliation logicielle, journal, CLI — avant
d'avoir besoin d'un labo. La confiance se construit avant la première écriture.

**Dès la Phase 2, tout se teste dans la VM.** Voir [`06-VM-DE-LABO.md`](06-VM-DE-LABO.md).

## Installation

### 1. Rust

```powershell
winget install Rustlang.Rustup
rustup default stable
rustup target add x86_64-pc-windows-msvc x86_64-unknown-linux-musl
rustup component add rustfmt clippy
```

Le fichier `rust-toolchain.toml` à la racine du dépôt épingle déjà le canal et les
cibles : un `rustup` récent les installera tout seul au premier `cargo build`.

### 2. Outils de compilation Microsoft

La cible MSVC a besoin du linker de Visual Studio. Les Build Tools suffisent, pas
besoin de l'IDE complet :

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

Dans l'installeur, cocher **« Développement Desktop en C++ »** et le **SDK Windows**.

### 3. Cross-compilation de l'agent Linux

```powershell
cargo install cargo-zigbuild
winget install zig.zig
```

`cargo-zigbuild` utilise Zig comme linker multiplateforme : c'est la voie la plus
simple pour produire un ELF statique musl depuis Windows, sans Docker ni conteneur.

Alternative si tu as déjà Docker : `cargo install cross` puis
`cross build --target x86_64-unknown-linux-musl`.

### 4. Le reste

```powershell
winget install Git.Git Microsoft.VisualStudioCode
```

### 5. Vérifier

```powershell
.\scripts\dev-setup.ps1
```

Ce script **ne modifie rien** : il vérifie et rapporte. C'est cohérent avec le
principe P2 du projet — même les scripts de développement montrent avant de faire.

## Extensions VS Code utiles

| Extension | Pourquoi |
|---|---|
| `rust-lang.rust-analyzer` | indispensable |
| `tamasfe.even-better-toml` | les `Cargo.toml` et `rust-toolchain.toml` |
| `ms-vscode.powershell` | les scripts de `scripts/` |
| `redhat.vscode-yaml` | valide `workstation.yaml` contre le JSON Schema de `schema/` |
| `usernamehw.errorlens` | voir les erreurs clippy en ligne |

Pour que la validation YAML fonctionne, ajouter dans les réglages du workspace :

```json
{
  "yaml.schemas": {
    "./schema/workstation.schema.json": ["workstation.yaml", "base.yaml", "schema/examples/*.yaml"]
  },
  "rust-analyzer.check.command": "clippy"
}
```

## Boucle de développement

### Phase 0 — sur l'hôte, sans risque

```powershell
cargo test --workspace          # 34 tests, tous portables
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ks-cli -- scan
cargo run -p ks-cli -- status
cargo run -p ks-cli -- explain inventory.cpu.cores
```

### L'agent Linux

```powershell
.\scripts\build-agent.ps1                  # produit dist\ks-agent
wsl -d Debian -- /mnt/d/src/keystone/dist/ks-agent
```

### Phase 2 et suivantes — dans la VM

```powershell
cargo build --release
.\scripts\lab-vm-reset.ps1                 # retour au point de contrôle « clean »
# copier les binaires dans la VM, exécuter, observer
.\scripts\lab-vm-reset.ps1                 # et on recommence
```

## Pièges

**Antivirus et dossier `target`.** Defender analyse en temps réel les centaines de
milliers de fichiers que Rust produit dans `target\`, et ça peut doubler le temps de
build. C'est exactement le problème que l'exigence D11-01 du produit décrit — et la
règle D11-02 s'applique à nous aussi : l'exclusion doit être **ciblée sur `target\`
seulement**, jamais sur le dépôt entier ni sur `D:\src`, et notée quelque part avec
sa raison. On ne s'accorde pas à soi-même ce qu'on refuse à l'utilisateur.

**Chemins longs.** Rust + Windows + chemins profonds : activer le support des chemins
longs une fois pour toutes.

```powershell
git config --global core.longpaths true
# et, en administrateur :
New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" `
  -Name LongPathsEnabled -Value 1 -PropertyType DWORD -Force
```

**Fins de ligne.** `.gitattributes` n'existe pas encore dans le dépôt ; `.editorconfig`
impose LF partout sauf pour les `.ps1` (CRLF). Si tu vois des diffs entiers sans
changement réel, c'est ça.

**Ne pas installer le service sur l'hôte.** `ks-broker` s'enregistre comme service
Windows. Le tenter sur ta machine principale, c'est mettre un composant privilégié
en cours de développement dans le démarrage de ton poste de travail. La VM existe
pour ça.

## Si ton poste est géré par l'entreprise

Deux capacités peuvent être bloquées par politique, et elles sont toutes deux
nécessaires à partir de la Phase 2 :

- **la création de VM Hyper-V** — sans elle, pas de labo ;
- **`bcdedit /set testsigning on`** — sans lui, pas de chargement de modules signés
  par un certificat de développement.

Dans ce cas, deux options : demander une dérogation pour un poste de développement,
ou héberger le labo ailleurs (second poste, serveur de virtualisation). Le
développement de `ks-core`, `ks-cli` et `ks-agent-linux` reste possible sans rien de
tout ça — ce qui couvre toute la Phase 0 et une bonne part de la Phase 1.
