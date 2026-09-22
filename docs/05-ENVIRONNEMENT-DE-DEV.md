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

Il tourne sur **Windows PowerShell 5.1**, celui présent sur toute machine Windows.
C'est délibéré : un script d'amorçage qui exigerait PowerShell 7 ne pourrait pas
s'exécuter tant que l'outillage est incomplet, c'est-à-dire précisément quand on en
a besoin. PowerShell 7 figure donc parmi ce qu'il **contrôle**, pas parmi ce qu'il
exige — il reste recommandé pour le confort interactif.

> **Encodage.** Les scripts sont en UTF-8 **avec BOM**. Sans lui, PowerShell 5.1 les
> lit comme de l'ANSI : les accents et les tirets cadratin deviennent du charabia, et
> le script ne compile même plus. `.editorconfig` impose `charset = utf-8-bom` sur
> les `.ps1` pour cette raison.

### Hyper-V : ce que le script constate, et ce que ça veut dire

Un hyperviseur peut tourner sur ta machine sans que le rôle Hyper-V soit installé :
WSL2 et Docker Desktop s'appuient sur la « plateforme de machine virtuelle », qui
suffit à leurs besoins mais **pas à créer une VM de labo**.

Le script distingue les deux cas. Sans élévation, il s'appuie sur la présence du
module de gestion (`Get-VM`), qui n'est livré qu'avec le rôle complet.

Un Hyper-V absent n'est pas un problème avant la **Phase 2** : les phases 0 et 1
n'écrivent rien et tournent sans risque sur la machine principale.

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
cargo test --workspace          # 288 tests au total, tous portables
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ks-cli -- scan
cargo run -p ks-cli -- status
cargo run -p ks-cli -- explain inventory.cpu.cores
```

### Phase 1 — adopter l'état lu, puis mesurer l'écart

```powershell
cargo run -p ks-cli -- import -o w.yaml   # écrit le fichier, n'écrase jamais sans --force
cargo run -p ks-cli -- diff --config w.yaml
```

Sur une machine qui n'a pas bougé entre les deux commandes, `diff` publie **zéro
écart**. Ce qu'il publie en incomparables, ce sont les items déclarables dont la
lecture a échoué : sur la machine de référence, les trois exclusions Defender,
sous une clé protégée par ACL qu'une CLI non élevée ne lit pas.

`import` **affiche** la commande git à exécuter et ne la lance jamais (ADR-0017) :
un `git commit` déclenche les crochets du dépôt, donc l'exécution d'un fichier du
disque que Keystone n'a pas choisi.

### Le schéma JSON se régénère, il ne s'édite pas

`schema/workstation.schema.json` est une **sortie** des types de `ks-cli`, plus un
document (ADR-0010, décision n° 2). Après toute modification d'`EtatDesire`, de
`Metadata` ou d'`EcartAccepte` :

```powershell
cargo run -q -p ks-cli --example generer-schema > schema/workstation.schema.json
```

Deux garde-fous refusent l'écart, et ils ne voient pas la même chose :
`le_schema_versionne_est_celui_que_les_types_produisent` le signale dès
`cargo test`, et le travail « Schéma workstation.yaml » de la CI régénère puis
refuse le moindre `git diff` — y compris une fin de ligne, que `.gitattributes`
fixe à LF.

L'exemple `schema/examples/workstation.yaml` est validé **deux fois**, et les deux
disent des choses différentes : par le validateur JSON Schema, et par
`EtatDesire::lire`, c'est-à-dire par le produit. Un exemple qui ne passerait que
le premier est exactement l'état dont la Phase 1 sort.

### La coque de bureau

`ui/` est un **workspace séparé** (ADR-0012) : il ne profite d'aucune des commandes
ci-dessus et porte les siennes. Les trois mêmes, depuis `ui/` :

```powershell
cd ui
cargo test --workspace          # 30 tests, dont les barrières du poste de pilotage
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run -p ks-ui              # ouvre la fenêtre, collecte, affiche
```

Le frontend vit dans `ui/ks-ui/web/`. Deux choses à savoir avant d'y toucher :

- **aucune valeur ne s'écrit dans le balisage.** Les emplacements portent
  `data-mesure` et restent vides ; `app.js` les remplit depuis le pont, ou écrit
  « non relevé ». Un chiffre laissé dans `index.html` fait échouer la suite ;
- **les couleurs et les piles de polices sont recopiées de `design/tokens.css`**,
  et une étape de CI compare les deux. Une teinte substituée à l'œil casse la CI,
  ce qui est le but.

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

## Le sondage horaire, et pourquoi Keystone ne le crée pas

Le critère de sortie de la Phase 1 est « la dérive suivie pendant sept jours sans
faux positif inexpliqué ». Sept jours de temps mural : il faut donc que quelque
chose appelle `ks scan --record` régulièrement.

**Keystone ne crée pas cette tâche, et ne la créera pas.** Créer une tâche
planifiée est une écriture système. Les phases 0 et 1 n'en font aucune, et cette
règle n'a pas d'exception pour la commodité de son auteur — c'est même le genre
d'exception qui, une fois accordée, ne se retire plus.

La commande est donnée ici pour que tu la crées toi-même, en sachant ce qu'elle
fait :

```powershell
# Un scan par heure, sous ton compte, sans élévation.
# Adapte le chemin si ton binaire est ailleurs.
$ks = "$PWD	arget
elease\ks.exe"

$action     = New-ScheduledTaskAction  -Execute $ks -Argument "scan --record"
$declencheur = New-ScheduledTaskTrigger -Once -At (Get-Date) `
                 -RepetitionInterval (New-TimeSpan -Hours 1)
$reglages   = New-ScheduledTaskSettingsSet -StartWhenAvailable `
                 -DontStopIfGoingOnBatteries -AllowStartIfOnBatteries

Register-ScheduledTask -TaskName "Keystone — sondage" `
  -Action $action -Trigger $declencheur -Settings $reglages `
  -Description "Relève l'état du poste et l'enregistre. Lecture seule."
```

Pour la retirer :

```powershell
Unregister-ScheduledTask -TaskName "Keystone — sondage" -Confirm:$false
```

### Ce que le sondage écrit, et ce qu'il n'écrit pas

Il écrit **uniquement** dans `%LOCALAPPDATA%\Keystone\journal.sqlite` : l'entrée
de journal du scan, et les intervalles d'observation. Aucune valeur du système
n'est touchée — c'est vérifiable, la bannière de `ks scan` l'annonce et aucun
collecteur ne détient de canal d'écriture.

### Le volume, mesuré

L'encodage est par **intervalles** et non par échantillons : un scan qui revoit la
même valeur avance une date, il n'insère rien. Onze scans consécutifs sur une
machine au repos ont produit **108 lignes**, pas 1 188 (mesuré le 2026-08-03).
La taille du magasin est donc proportionnelle au **changement**, pas au temps —
c'est ce qui rend la rétention inutile plutôt que reportée.

### Une heure, et pas cinq minutes

Un sondage plus fréquent ne rend pas la dérive plus visible : il rend seulement
l'intervalle d'attribution plus étroit, ce qui n'a d'intérêt qu'une fois
l'attribution par événement disponible — donc pas en Phase 1. Il consomme en
revanche du temps de veille et de la batterie à chaque passage. Une heure est le
compromis retenu ; la valeur se change dans la commande ci-dessus.

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

**Fins de ligne.** `.gitattributes` fixe les fins de ligne du dépôt ; `.editorconfig`
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
