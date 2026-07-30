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
| `ks-core` | vocabulaire : `Item`, `Drift`, `Plan`, `Action`, `Snapshot`, `JournalEntry` | portable | aucun | ✅ 19 tests |
| `ks-collectors` | collecte **lecture seule** | portable + extensions Windows | aucun | ✅ 4 tests, socle portable |
| `ks-cli` | la CLI `ks`, surface de référence | Windows (et Linux pour le dev) | aucun | ✅ `scan`/`status`/`explain` |
| `ks-broker` | service privilégié | **Windows uniquement** | élevé | 🔨 énumération des verbes + barrière de test |
| `ks-agent-linux` | agent satellite | Linux musl | aucun | 🔨 scan local |

## Les invariants portés par le typage

Plutôt que par la relecture humaine — parce que la relecture humaine fatigue et que
le compilateur, non.

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
