# 03 — Architecture

> Ce document décrit *comment* le produit est bâti. Le *quoi* et le *pourquoi* sont
> dans [`01-CAHIER-DES-CHARGES.md`](01-CAHIER-DES-CHARGES.md). Les décisions
> individuelles et leurs alternatives écartées sont dans [`adr/`](adr/).

## Vue d'ensemble

```
┌─────────────────────────────────────────────────────────────────┐
│  CLIENTS — non privilégiés                                      │
│                                                                 │
│   ks-cli (ks)        UI Tauri/WebView2        vue mobile (P2)    │
│        │                    │                       │           │
└────────┴────────────────────┴───────────────────────┴───────────┘
                              │
              gRPC sur named pipe · jeton par session
              aucun port TCP en écoute (SEC-12)
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│  ks-broker — SERVICE WINDOWS, LE SEUL COMPOSANT ÉLEVÉ            │
│                                                                 │
│   · API de verbes typés et énumérés (SEC-02)                    │
│   · Autorisation · Journalisation · Limitation de débit          │
│   · Contrôle d'intégrité au démarrage (SEC-07)                  │
│   · Moteur de convergence · Ordonnanceur · Instantanés           │
└───┬──────────────┬──────────────────┬───────────────────────────┘
    │              │                  │
┌───▼──────────┐ ┌─▼──────────────┐ ┌─▼─────────────────────────┐
│ COLLECTEURS  │ │  EXÉCUTEURS    │ │  AGENTS SATELLITES         │
│ lecture seule│ │  PowerShell 7  │ │  ks-agent-linux            │
│ WASM (tiers) │ │  CIM/WMI       │ │  · WSL2 : socket relayé    │
│ natif (nous) │ │  WinAPI        │ │  · VM   : AF_HYPERV        │
└──────────────┘ └────────────────┘ └───────────────────────────┘
    │
┌───▼─────────────────────────────────────────────────────────────┐
│  PERSISTANCE LOCALE                                             │
│   SQLite (WAL) — état, historique, inventaire                    │
│   Journal append-only, chaîné par empreinte                      │
│   Exporteur Prometheus (désactivé par défaut)                    │
└───┬─────────────────────────────────────────────────────────────┘
    │
┌───▼─────────────────────────────────────────────────────────────┐
│  ANCRES EXTERNES — hors de portée d'un attaquant local           │
│   · journal expédié en écriture seule (SEC-04)                   │
│   · battement de cœur attendu ; le silence est une alerte (SEC-05)│
│   · référence d'attestation TPM / PCR (SEC-06)                   │
│   · dépôt git de workstation.yaml                                │
└─────────────────────────────────────────────────────────────────┘
```

## Pourquoi cette forme

### Le broker est petit exprès

C'est le seul composant élevé, donc le seul dont la compromission est fatale. Sa
surface est réduite au minimum : une API de verbes énumérés, pas un interpréteur.
Tout le reste — collecte, présentation, orchestration côté client — vit hors
privilège.

Le corollaire est inconfortable et assumé : **ajouter une fonctionnalité coûte plus
cher** qu'avec un service généraliste, parce que chaque verbe doit être conçu, typé,
justifié, et savoir se simuler et s'annuler. C'est le prix du modèle de menace.

### Les collecteurs sont isolés

Un collecteur qui plante ne doit pas emporter le broker, et un collecteur tiers ne
doit pas hériter de ses privilèges. D'où le bac à sable WASM avec capacités
déclarées, et le budget d'empreinte par collecteur (NF-01) : celui qui dépasse est
mis en veille et signalé, il ne ralentit pas la machine en silence.

### Les agents parlent le même langage que l'hôte

`ks-agent-linux` produit exactement le même schéma d'items que les collecteurs de
l'hôte, parce qu'il partage le crate `ks-collectors`. C'est ce qui rend la vue
consolidée multi-OS possible sans dupliquer la logique — et c'est aussi pourquoi
`ks-core` et `ks-collectors` ne dépendent de rien de spécifique à une plateforme.

### Hyper-V Socket plutôt que le réseau

Pour joindre une VM, on utilise `AF_HYPERV`, pas TCP. La raison est simple : on veut
pouvoir atteindre une VM **au moment où son réseau est cassé** — c'est-à-dire quand
on a le plus besoin de la joindre. Même raisonnement pour PowerShell Direct dans la
boucle de développement.

### Les ancres externes existent parce que l'agent local n'est pas crédible

C'est le point d'architecture le plus important du projet, et le moins intuitif :

> Un agent qui tourne sur la machine qu'il surveille **ne peut pas être cru** quand il
> la déclare saine.

Un attaquant avec SYSTEM peut réécrire le journal local, faire mentir les
collecteurs, ou simplement tuer le service et laisser un « tout va bien » figé. Trois
mécanismes répondent à cela, et aucun ne dépend de l'intégrité de la machine :

1. le journal est **déjà parti** vers une destination en écriture seule ;
2. l'**absence** de battement de cœur est une alerte, pas un silence neutre ;
3. les **PCR du TPM** sont comparés à une référence conservée ailleurs.

## Cartographie des crates

| Crate | Rôle | Plateforme | Privilège | État |
|---|---|---|---|---|
| `ks-core` | vocabulaire : `Item`, `Nature`, `Desire`, `Drift`, `Plan`, `Action`, `Snapshot`, `JournalEntry` | portable | aucun | ✅ 39 tests unitaires |
| `ks-collectors` | collecte **lecture seule** | portable ; matériel, inventaire logiciel, posture par le registre **et état effectif par WMI** — TPM, BitLocker et SMART restent hors de portée sans élévation (Phase 2) | aucun | ✅ 61 tests unitaires, 4 collecteurs |
| `ks-cli` | la CLI `ks`, surface de référence | Windows (et Linux pour le dev) | aucun | ✅ `scan`/`status`/`explain`/`journal`/`report` + lecteur de `workstation.yaml`, 49 tests unitaires + **7 d'intégration** (binaire lancé en sous-processus) |
| `ks-broker` | service privilégié | Windows visé ; compile aussi ailleurs, sans effet | élevé | 🔨 verbes énumérés + **huit** barrières SEC-02 et SEC-03, 13 tests unitaires — aucun verbe implémenté |
| `ks-agent-linux` | agent satellite | Linux musl | aucun | 🔨 scan local, 4 tests unitaires |
| `ks-ui` | coque de bureau (ADR-0012) | Windows + WebView2 | aucun | ✅ affiche le **poste de pilotage** branché sur l'état réel, jamais les chiffres de la maquette · workspace **séparé**, 30 tests |

**173 tests au total** dans le workspace principal — `ks-ui` vit dans un workspace séparé et porte les siens (30), tous portables et tous exécutés — `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` et `cargo fmt --all --check`
passent. Ce n'était pas le cas au premier commit : rien n'avait alors jamais été
compilé, et les comptes annoncés étaient des déclarations.

## Ce que la coque affiche, et ce qu'elle refuse d'afficher

`ks-ui` porte le **poste de pilotage** décrit par
[`02-BRIEF-DESIGN.md`](02-BRIEF-DESIGN.md) : rail repliable (`Ctrl+B`), palette de
commandes (`Ctrl+K`) avec la commande `ks` équivalente affichée à côté de chaque
écran, chiffres tabulaires, mouvement réduit et contraste forcé traités. Le
balisage et le style viennent de la maquette rangée dans `design/` ; ses chiffres,
non.

Une seule règle gouverne cet écran : **aucun nombre affiché n'est inventé.** Chaque
valeur vient d'un item réellement relevé, ou l'écran dit qu'elle n'est pas
collectée, avec la raison et ce qui la rendrait disponible.

| Écran | Ce qu'il affiche | D'où ça vient |
|---|---|---|
| Vue d'ensemble | décompte d'items, constat de posture, quatre tuiles, répartition par domaine | `Inventory::collect_all()` |
| Sécurité | les items du domaine, par famille, les illisibles comptés **et nommés** | `security.*` |
| Dérive | pourquoi aucune comparaison n'a lieu, et les quatre verdicts | `Item::verdict()` |
| Mises à jour | pourquoi le domaine n'est pas collecté | — |
| Machine | nom, système, noyau, cœurs, mémoire, temps de fonctionnement | `inventory.host.*`, `inventory.os.*`, `inventory.cpu.*`, `inventory.memory.*`, `inventory.uptime_seconds` |
| Logiciels | agrégats d'attribution, applications sans gestionnaire identifié | `inventory.software.*` |
| Espace | taux d'occupation par volume, jauge **et** tableau | `space.volume[…].used_percent` |
| WSL et VM | une carte par distribution | `virtualization.wsl[…]` |
| Rapport | le HTML de `ks report`, dans un cadre `sandbox` sans permission | `ks_cli::rapport::construire` |

Trois éléments du poste de pilotage **ne sont pas calculables aujourd'hui**, et
l'écran le dit plutôt que de les remplir :

- **la posture composite.** Elle se calcule contre une référence, et aucun état
  désiré n'est chargé. **L'anneau du brief §4.1 n'est donc pas dessiné du tout** :
  un arc partiel serait une valeur, et un anneau vide de 280 px n'est qu'un
  cercle qui attend un chiffre absent. À sa place, le dessin que la marque tient
  pour ce moment — `design/marque/etat-premier-lancement.svg`, l'arc dont la clé
  de voûte n'est pas encore posée — et le constat en toutes lettres. Le rôle
  `meter` est absent lui aussi, faute de mesure à annoncer (P6, D2-02). L'anneau
  reprendra sa place le jour où la posture sera une mesure ;
- **la dérive.** Sans état désiré il n'y a pas zéro écart, il n'y a pas de
  comparaison. C'est ce que `Verdict::Incomparable` existe pour empêcher, transposé
  à l'écran ;
- **les mises à jour.** Le domaine D3 n'est pas collecté.

Ces trois-là passent par un type sans champ numérique, `NotComputable`, qui porte
les trois temps de la voix du produit — ce qui s'est passé, ce que ça implique, ce
qu'on peut faire — plus un `detail` technique qui se copie. La dérive va plus loin :
elle est une **énumération**, et la variante publiée tant qu'aucun item n'est
contraint ne contient littéralement pas de champ « écarts ». La page ne peut pas
afficher « 0 écart », parce que le zéro n'existe nulle part dans ce qu'elle reçoit.

Trois barrières le tiennent, toutes éprouvées par falsification : un emplacement de
valeur du balisage qui porterait un chiffre casse la suite, une mesure écrite à la
main dans une phrase aussi, et une cinquième valeur de `Verdict` casse la
**compilation** de la coque avant qu'un test s'exécute.

## Les invariants portés par le modèle

Plutôt que par la relecture humaine — parce que la relecture humaine fatigue et que
le compilateur, non.

**Deux mécanismes distincts, et il faut les distinguer honnêtement.**

*Rendus impossibles par le typage* — ces états ne se compilent pas. `Item.provenance`
et `Item.observed_at` ne sont pas des `Option` : un item sans origine ni horodatage
est inconstructible. `Item.nature` non plus, et sans valeur par défaut : un
collecteur ne peut pas produire un item sans avoir décidé s'il a vocation à être
déclaré (ADR-0009). `Nature` n'est délibérément pas `#[non_exhaustive]`, pour que
ses consommateurs gardent un `match` exhaustif sans bras `_` : ajouter une
cinquième vocation casse la compilation là où l'on décide quelque chose d'une
nature, avant qu'un test s'exécute. `DriftStatus::Accepted` porte `reason`, `expires`, `decided_by`
et `decided_at` en champs obligatoires : une exception sans motif ni date
d'expiration n'existe pas. `Snapshot` et `BackupSet` sont deux types sans
conversion : on ne peut pas passer l'un pour l'autre, ce qui était l'angle mort
de la première conception.

Les paramètres des verbes d'écriture de configuration relèvent aussi de cette
catégorie depuis l'[ADR-0006](adr/0006-fermer-les-verbes-a-parametres-libres.md) :
`ManagedService`, `ManagedSetting`, `StartupType` et `SettingValue` sont des
énumérations **sans champ**, donc incapables de transporter un chemin.

**Ce paragraphe a été faux deux fois, dans les deux sens.**

Il a d'abord rangé `Verb::AddDefenderExclusion` parmi les verbes typés alors que
ses trois champs étaient des `String` : le type garantissait leur *présence*, pas
leur *validité*, et `expires: "hier"` compilait. C'était la sur-affirmation que
`rust.md` interdit — ne jamais écrire « porté par le typage » pour ce qui ne
l'est pas.

Puis il a continué de le dire **après** la correction. Les chaînes libres ont été
fermées : `ExclusionPath` et `Expiry` sont des newtypes à champ privé portant
`#[serde(try_from = "String")]`, `Expiry::depuis("hier", …)` renvoie une erreur,
et un test le vérifie. Le commit qui a payé cette dette a mis à jour
l'ADR-0006 — dont la section s'intitule « La dette — payée » — mais pas ce
document. Un seul document sur deux corrigé recrée exactement la contradiction
que la règle « le code et sa documentation partent dans le même commit » existe
pour empêcher.

Ce qui reste en `String`, et pourquoi c'est légitime : `reason`, un motif écrit
par un humain pour un humain, qu'aucun type ne saurait valider utilement, et
`domain`, un filtre d'affichage sans effet. Le détail est dans
[l'ADR-0006](adr/0006-fermer-les-verbes-a-parametres-libres.md).

*Vérifiés à l'exécution, et testés* — les fonctions du tableau ci-dessous renvoient
un `Result` ; elles refusent, elles n'empêchent pas d'appeler. `Capabilities` est une
structure de booléens, pas un typestate : rien dans le système de types n'oblige un
futur exécuteur à consulter `ensure_appliable` avant d'écrire. C'est un contrôle
d'admission, bien couvert par les tests — pas une garantie du compilateur.

La distinction est sans conséquence aujourd'hui, puisqu'aucun exécuteur n'existe.
Elle en aura une en Phase 2, et c'est le bon moment pour décider si ces invariants
doivent migrer vers un typestate (`Plan<Simulated>` distinct de `Plan<Draft>`) ou
rester des contrôles d'admission assumés.

| Invariant | Où il vit |
|---|---|
| Une action qui ne sait pas se simuler ne peut pas être appliquée (**P2**) | `Action::ensure_appliable` |
| Une action qui ne sait pas s'annuler ne peut pas être automatisée (**P3**) | `Action::ensure_schedulable` |
| Un plan non simulé ne peut pas être appliqué (**P2**) | `Plan::ensure_appliable` |
| Une vague qui écrit sans instantané est détectable (**P3**) | `Plan::waves_without_safety_net` |
| Un conflit MDM n'est jamais convergeable (**P10**) | `Drift::is_convergeable` |
| Une acceptation expirée redevient un écart, sans intervention (**D2-06**) | `Drift::is_expired_acceptance` |
| Un changement sans auteur est un signal de sécurité (**D2-05**) | `Provenance::is_security_signal` |
| Une exclusion Defender exige raison **et** expiration (**D11-02**) | `Verb::AddDefenderExclusion` — champs obligatoires |
| Un item sans finalité ni risque documenté n'est pas construisible (**P6**) | constructeur `observed()` |
| Une sauvegarde jamais restaurée n'est pas digne de confiance (**D6-03**) | `BackupSet::is_trustworthy` |
| Aucun verbe ne transporte d'exécution arbitraire (**SEC-02**) | test `aucun_verbe_ne_transporte_dexecution_arbitraire` |

Ce dernier point mérite une note : c'est un test volontairement grossier, qui inspecte
le nom des champs sérialisés. Il ne prouve rien formellement. Son rôle est de **faire
casser la CI** si quelqu'un ajoute `RunCommand`, pour que la discussion ait lieu avant
la fusion plutôt qu'après l'incident.

## Modèle de plugin

Chaque module déclare dans son manifeste :

```
reads              quelles sources il lit
writes             quelles cibles il écrit (vide pour un collecteur)
privileges         ce dont il a besoin
supports_dryrun    sait-il montrer avant de faire ?
supports_rollback  sait-il défaire ?
idempotent         deux applications = même état ?
platform           windows | linux | portable
signature          signature du module
```

Deux règles sont vérifiées **au chargement** par le broker, pas à l'exécution :

- pas de `supports_dryrun` → le module ne peut pas être appelé en mode `apply` ;
- pas de `supports_rollback` → le module ne peut pas être appelé par l'ordonnanceur.

## Flux d'une convergence

```
  ks converge                    (aucun --apply : simulation, P2)
      │
      ├─ 1. charger workstation.yaml + valider contre le JSON Schema
      ├─ 2. collecter l'état réel (lecture seule)
      ├─ 3. calculer les écarts, attribuer chaque changement à sa source
      ├─ 4. écarter les conflits MDM (non convergeables, D12-03)
      ├─ 5. écarter les acceptations non expirées (D2-06)
      ├─ 6. bâtir le plan : vagues ordonnées, dépendances respectées (D3-08)
      ├─ 7. vérifier les capacités de chaque action (P2 et P3)
      ├─ 8. annoncer le filet de sécurité et le coût du retour arrière
      └─ 9. AFFICHER — et s'arrêter là
             │
             └─ ks converge --apply   (seulement si l'étape 9 a eu lieu)
                    │
                    ├─ 10. prendre les instantanés
                    ├─ 11. appliquer vague par vague
                    ├─ 12. après chaque vague : jouer les tests de fumée
                    ├─ 13. échec → RESTAURER + épingler le fautif + raconter
                    └─ 14. journaliser, expédier, mettre à jour la posture
```

L'étape 13 est le moment de vérité **M5** du brief de design. Elle se traite comme un
**succès**, pas comme une erreur : c'est l'instant où le produit gagne la confiance
de son utilisateur.

## Ce que l'architecture ne résout pas

Honnêteté nécessaire, et déjà actée au §4.3 du cahier des charges : rootkit noyau ou
firmware, attaque purement en mémoire, vol de jeton par canal légitime, hyperviseur
compromis, attaque physique. Les ancres externes permettent de **constater une
incohérence**, pas de voir le malware.

Voir [`04-MODELE-DE-MENACE.md`](04-MODELE-DE-MENACE.md).
