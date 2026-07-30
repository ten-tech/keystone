# Documentation Keystone — index

Les documents sont numérotés **dans l'ordre où il faut les lire**.

| # | Document | À lire quand |
|---|---|---|
| — | [`../README.md`](../README.md) | en premier, toujours |
| 01 | [Cahier des charges](01-CAHIER-DES-CHARGES.md) | pour comprendre *ce que* fait le produit et pourquoi. Contient les exigences numérotées, référencées partout ailleurs. |
| 02 | [Brief de design](02-BRIEF-DESIGN.md) | pour comprendre *comment* le produit se présente. Contient le prompt prêt à coller pour Claude Design. |
| 03 | [Architecture](03-ARCHITECTURE.md) | avant d'écrire du code dans `crates/`. |
| 04 | [Modèle de menace](04-MODELE-DE-MENACE.md) | **obligatoire** avant de toucher à `ks-broker`. |
| 05 | [Environnement de développement](05-ENVIRONNEMENT-DE-DEV.md) | pour installer sa machine. Répond à « je code où ? ». |
| 06 | [VM de labo](06-VM-DE-LABO.md) | avant la Phase 2, quand le code commence à écrire. |
| 07 | [Feuille de route](07-FEUILLE-DE-ROUTE.md) | pour savoir quoi coder maintenant. |
| 08 | [Conventions](08-CONVENTIONS.md) | avant la première contribution. |
| 09 | [Glossaire](09-GLOSSAIRE.md) | dès qu'un mot du projet est ambigu. |
| — | [`adr/`](adr/) | décisions d'architecture, une par fichier, jamais réécrites. |

## Les autres répertoires

| Répertoire | Contenu |
|---|---|
| [`../design/`](../design/) | maquette interactive, tokens CSS, note de validation de la palette |
| [`../schema/`](../schema/) | JSON Schema de `workstation.yaml` + exemple complet commenté |
| [`../scripts/`](../scripts/) | PowerShell : création du labo, vérification de la toolchain, cross-compilation |

## Comment lire les références

Les exigences sont citées par leur identifiant, et il est stable :

- **D3-07** → domaine 3 (mises à jour), exigence 7 → §7 du cahier des charges
- **SEC-02** → exigence de sécurité 2 → §6 du cahier des charges
- **NF-05** → exigence non fonctionnelle 5 → §8
- **A11** → critère d'acceptation 11 → §12
- **P3** → principe directeur 3 → §3 (et README)
- **M5** → moment de vérité 5 → §5 du brief de design

Quand un commentaire de code cite `(D3-07)`, il renvoie à cette exigence. Si tu
changes un comportement, vérifie l'exigence — et si l'exigence est fausse, corrige
l'exigence dans le même commit que le code.

## Règle de tenue de la documentation

**Un comportement non documenté est un défaut.** Les documents ne sont pas un
livrable de fin de projet : le cahier des charges et les ADR sont modifiés dans le
même commit que le code qu'ils décrivent. Une documentation qui décrit un produit
qui n'existe plus est pire que pas de documentation.
