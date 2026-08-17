# Keystone

**Plan de contrôle déclaratif pour poste de travail d'ingénieur.**
Windows 11 · WSL2 · Hyper-V

Keystone décrit l'état voulu d'une machine dans un fichier versionné, mesure l'écart en continu,
propose une convergence **simulée**, l'applique **sous filet de sécurité**, et journalise tout de
manière inaltérable.

> **Une machine d'ingénieur doit être un artefact reproductible, pas un objet unique et fragile.**

---

## Les cinq règles qui gouvernent tout le projet

Si une décision technique contredit une de ces règles, c'est la décision qui est fausse.

1. **État désiré, pas action ponctuelle.** La source de vérité est `workstation.yaml`, versionné en git.
2. **Simulation par défaut.** `--dry-run` est le comportement implicite. `--apply` est toujours explicite.
3. **Rien d'irréversible sans filet.** Si Keystone ne sait pas fabriquer le retour arrière d'une opération, il ne l'automatise pas.
4. **Sans interface graphique d'abord.** Le cœur est un service + une CLI. L'UI est un client parmi d'autres.
5. **Local, point final.** Aucun compte, aucune télémétrie sortante, aucun charlatanisme.

Et une sixième, pour l'interface : **le calme est la fonctionnalité.** La récompense offerte à
l'utilisateur est un écran silencieux, pas une stimulation.

---

## Où commencer

| Je veux… | Fichier |
|---|---|
| **comprendre le projet en 10 minutes** | ce README, puis [`docs/00-INDEX.md`](docs/00-INDEX.md) |
| le périmètre et les exigences numérotées | [`docs/01-CAHIER-DES-CHARGES.md`](docs/01-CAHIER-DES-CHARGES.md) |
| la direction UI/UX et le prompt pour Claude Design | [`docs/02-BRIEF-DESIGN.md`](docs/02-BRIEF-DESIGN.md) |
| **installer mon environnement de dev** | [`docs/05-ENVIRONNEMENT-DE-DEV.md`](docs/05-ENVIRONNEMENT-DE-DEV.md) |
| créer la VM de labo (à faire avant la Phase 2) | [`docs/06-VM-DE-LABO.md`](docs/06-VM-DE-LABO.md) |
| savoir quoi coder maintenant | [`docs/07-FEUILLE-DE-ROUTE.md`](docs/07-FEUILLE-DE-ROUTE.md) |
| voir la maquette | ouvrir [`design/keystone-cockpit.html`](design/keystone-cockpit.html) dans un navigateur |

---

## Démarrage rapide

```powershell
# 1. Vérifier que la toolchain est complète (ne modifie rien)
.\scripts\dev-setup.ps1

# 2. Compiler
cargo build --workspace

# 3. Premier scan — LECTURE SEULE, aucune écriture système
cargo run -p ks-cli -- scan
cargo run -p ks-cli -- status

# 4. Adopter l'état lu comme référence, puis mesurer l'écart (Phase 1)
cargo run -p ks-cli -- import -o workstation.yaml
cargo run -p ks-cli -- diff --config workstation.yaml
cargo run -p ks-cli -- accept <item> --reason "..." --until 2026-11-30   # simule ; --apply pour écrire
```

> **Rien n'est écrit sur l'état de la machine** : ni registre, ni service, ni politique, ni fichier
> d'un autre programme. Tu peux faire tourner tout cela sur ton poste principal sans risque :
> aucun collecteur n'a le droit d'écrire, et le broker n'est pas encore dans la boucle.
> *La formule « rien du tout » figurait ici, et elle était fausse depuis la Phase 0.5 — une règle
> fausse ne protège plus rien. Les trois cercles sont énoncés par l'ADR-0020.*
> Les fichiers que Keystone dépose sont ceux qu'on lui demande — `ks report` et `ks import` —
> jamais par-dessus un existant sans `--force`. `ks accept --apply` est la seule commande qui
> **modifie** un fichier existant : elle insère une tolérance dans `workstation.yaml` et laisse
> tout le reste octet pour octet, ce qu'un test vérifie par soustraction. Sans `--apply`, elle
> n'écrit rien du tout. Et Keystone n'exécute jamais git (ADR-0017).
> À partir de la Phase 2 (convergence), tout se teste dans la VM de labo — voir
> [`docs/06-VM-DE-LABO.md`](docs/06-VM-DE-LABO.md).

---

## Structure du dépôt

```
keystone/
├── docs/               toute la documentation, numérotée dans l'ordre de lecture
│   └── adr/            décisions d'architecture, une par fichier, immuables
├── design/             maquette interactive + tokens de design + validation de palette
│   └── marque/         identité visuelle : logomark, icônes, états illustrés
├── schema/             JSON Schema de workstation.yaml, GÉNÉRÉ depuis ks-cli + exemple
├── crates/
│   ├── ks-core/        types du modèle de données — Item, Drift, Plan, Snapshot, Journal
│   ├── ks-cli/         la CLI `ks` — surface de référence du produit
│   ├── ks-collectors/  collecteurs en lecture seule (Phase 0)
│   ├── ks-broker/      service Windows privilégié (Phase 2) — le composant sensible
│   └── ks-agent-linux/ agent pour les distros WSL2 et les VM Linux
├── ui/                 coque de bureau Tauri — workspace SÉPARÉ, verrou distinct
│   └── ks-ui/          le poste de pilotage, sur données réelles (ADR-0012, ADR-0013)
├── scripts/            PowerShell : labo, toolchain, cross-compilation
└── .github/workflows/  intégration continue
```

---

## Où se code quoi

| Composant | Machine de développement | Cible d'exécution |
|---|---|---|
| `ks-broker`, `ks-cli`, `ks-collectors`, UI | **Windows hôte, nativement** (`x86_64-pc-windows-msvc`) | Phase 0 : l'hôte · Phase 2+ : **VM de labo uniquement** |
| `ks-agent-linux` | Windows, en cross-compilation vers `x86_64-unknown-linux-musl` | une distro WSL2 |
| `ks-core` | n'importe où, il est portable | — |

**Ne développe pas la partie Windows depuis WSL.** La cible est MSVC, pas MinGW ; le débogueur,
l'Observateur d'événements et les traces ETW sont côté Windows ; et `/mnt/c` est lent.
Le raisonnement complet est dans [`docs/05-ENVIRONNEMENT-DE-DEV.md`](docs/05-ENVIRONNEMENT-DE-DEV.md).

---

## État d'avancement

| Phase | Objet | État |
|---|---|---|
| **0 — Observer** | collecteurs en lecture seule, inventaire, journal, CLI | ✅ critère de sortie atteint |
| 1 — Décrire | schéma yaml, import de l'existant, moteur de diff, dérive acceptée | ◐ tout est livré — `ks import`, `ks diff`, `ks accept` avec raison et échéance obligatoires, journal des décisions, schéma généré depuis les types. **Reste le critère de sortie**, qui ne se code pas : la dérive suivie sept jours sans faux positif inexpliqué. L'héritage de flotte est parti en Phase 6 avec D13, faute d'une seconde machine |
| 2 — Converger | instantanés, rollback, mises à jour orchestrées, sauvegarde | ○ à faire |
| 3 — Tenir | espace, posture de sécurité, ancres externes, isolement | ○ à faire |
| 4 — Vivre | profils, WSL/VM, réseau, docteur de dev, coexistence MDM | ○ à faire |
| 5 — Voir | interface Tauri, timeline, rapports, copilote local | ◐ socle livré en avance (ADR-0012, ADR-0013) : 9 écrans sur données réelles. Health Ring calculable, timeline forensique et copilote restent à faire |
| 6 — Essaimer | flotte, vue mobile, documentation, empaquetage | ○ à faire |

Détail et critères de sortie de chaque phase : [`docs/07-FEUILLE-DE-ROUTE.md`](docs/07-FEUILLE-DE-ROUTE.md).

---

## Avertissement de sécurité, à lire avant de contribuer

Keystone administre tout, avec les droits les plus élevés de la machine.
**Compromis, il constitue l'outil d'attaque le plus efficace imaginable sur ce poste.**

Ce n'est pas une formule. C'est la raison pour laquelle :

- le broker n'expose que des **verbes typés et énumérés** — jamais de primitive d'exécution libre ;
- l'interface et la CLI ne sont **pas privilégiées** ;
- tout est journalisé, et le journal est **expédié hors machine en écriture seule** ;
- **l'absence de battement de cœur est elle-même une alerte.**

Toute contribution qui élargit la surface du broker doit passer par une ADR.
Le modèle de menace complet, avec ses angles morts assumés, est dans
[`docs/04-MODELE-DE-MENACE.md`](docs/04-MODELE-DE-MENACE.md).

**Et ce que Keystone ne prétendra jamais faire :** détecter toute intrusion. Rootkit noyau ou
firmware, attaque purement en mémoire, vol de jeton par canal légitime, attaque physique — ces
limites sont structurelles, documentées, et affichées en première page de la documentation
utilisateur. Un outil de sécurité qui prétend tout voir est un mensonge.

---

## Licence

Apache-2.0 — voir [`LICENSE`](LICENSE).
