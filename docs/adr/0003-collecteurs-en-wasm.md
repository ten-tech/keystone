# ADR-0003 — Collecteurs tiers en WASM, collecteurs internes en natif

- **Statut** : Accepté
- **Date** : 2026-07-30
- **Exigences concernées** : SEC-07, NF-01, NF-03, NF-09

## Contexte

Les collecteurs sont nombreux et le resteront : chaque nouveau domaine en apporte. Ils
ont trois propriétés qui posent problème ensemble :

1. ils sont **en lecture seule**, donc peu risqués individuellement ;
2. ils sont **nombreux**, donc une part significative du code du produit ;
3. certains viendront de **tiers** — c'est souhaitable, un collecteur pour une carte
   réseau exotique ou un outil de dev de niche n'a pas à passer par nous.

Or un collecteur qui plante ne doit pas emporter le broker (NF-03), et un collecteur
tiers ne doit **jamais** hériter du privilège du broker.

Il y a aussi un problème d'empreinte : NF-01 fixe un budget global, et un collecteur
mal écrit qui sonde en boucle doit être identifié et mis en veille — pas ralentir la
machine en silence.

## Décision

**Deux régimes, selon l'origine.**

| Origine | Format | Exécution |
|---|---|---|
| **Interne** (ce dépôt) | natif, dans `ks-collectors` | en processus, revu par nous, testé en CI |
| **Tiers** | module **WASM** (wasmtime) | bac à sable, capacités déclarées, budget imposé |

Chaque module — interne ou tiers — porte un manifeste déclarant `reads`, `writes`,
`privileges`, `supports_dryrun`, `supports_rollback`, `idempotent`, `platform`,
`signature`.

Deux règles sont vérifiées **au chargement** par le broker, pas à l'exécution :

- un module sans `supports_dryrun` ne peut pas être appelé en mode `apply` ;
- un module sans `supports_rollback` ne peut pas être appelé par l'ordonnanceur.

Un collecteur déclare `writes: []`. **Un collecteur qui déclare une écriture n'est pas
un collecteur** et est refusé.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| **Tout en natif, dans le broker** | Le plus simple et le plus rapide, mais un collecteur tiers hériterait du privilège du composant élevé. Inacceptable : ça revient à faire signer par Keystone du code qu'on n'a pas lu. |
| **Tout en WASM, y compris nos collecteurs** | Uniformité séduisante, mais WASM n'a pas d'accès direct aux API Win32 : chaque appel WMI ou registre demanderait une fonction hôte dédiée. On finirait par construire une API hôte si large qu'elle deviendrait elle-même la surface d'attaque — on aurait déplacé le problème, pas résolu. |
| **Processus séparés avec jetons restreints** | Isolation réelle et éprouvée par le système. Écarté sur le coût : quelques dizaines de collecteurs, chacun dans son processus, contre un budget de 150 Mo résidents et 1 % de CPU. Le coût de création de processus sur Windows achève l'argument. À reconsidérer si WASM déçoit. |
| **Scripts PowerShell comme format de plugin** | Très accessible, et c'est un vrai argument pour l'adoption. Mais un plugin qui est un script est un plugin qui peut tout faire : on retomberait sur le problème que SEC-02 existe pour éviter. |
| **Modules dynamiques signés (DLL)** | Fonctionne, et c'est le modèle classique. Mais une DLL chargée en processus a le privilège du processus : la signature atteste l'origine, pas l'innocuité. |

## Conséquences

### Ce que ça nous donne

- Un collecteur tiers ne peut pas dépasser ses capacités déclarées, quelle que soit son
  intention.
- Un plantage est isolé : le broker survit, et signale le collecteur fautif.
- Le budget d'empreinte est imposable par module (NF-01) : celui qui dépasse est mis en
  veille et **signalé à l'utilisateur**, jamais silencieusement.
- Les collecteurs internes gardent l'accès direct et performant aux API Windows.

### Ce que ça nous coûte

- **Deux chemins de code à maintenir** pour la même abstraction. C'est le coût principal
  de cette ADR, et il est réel.
- L'API hôte offerte aux modules WASM doit être conçue avec le même soin que celle du
  broker — sinon on a juste déplacé la frontière de privilège d'un cran.
- Débogage d'un module WASM moins confortable qu'un binaire natif.

### Ce que ça ferme

Un plugin tiers ne pourra jamais faire ce qu'un collecteur interne fait, et c'est
volontaire. Si un besoin tiers légitime se heurte à cette limite, la bonne réponse est
d'élargir l'API hôte **par une ADR**, pas d'accorder une exception.

## À revoir si

- l'API hôte WASM dépasse une trentaine de fonctions — signe qu'on reconstruit un
  système d'exploitation en petit ;
- aucun collecteur tiers n'apparaît en dix-huit mois — le régime WASM serait alors une
  complexité payée pour rien, et la simplification serait de le retirer.
