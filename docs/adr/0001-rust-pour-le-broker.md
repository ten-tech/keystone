# ADR-0001 — Rust pour le broker privilégié

- **Statut** : Accepté
- **Date** : 2026-07-30
- **Exigences concernées** : SEC-01, SEC-02, SEC-07, NF-01, NF-08

## Contexte

Le broker est le seul composant élevé du produit. Il manipule les services Windows, le
registre, les politiques, Defender, BitLocker, le TPM, WSL et Hyper-V. Sa
compromission équivaut à la compromission totale de la machine — et pire, à une
compromission *légitime* aux yeux de l'utilisateur, puisqu'il fait confiance à
Keystone.

Trois contraintes pèsent sur le choix du langage :

1. il doit être **auditable** — la surface doit se relire en une session ;
2. il doit être **attestable** — un binaire natif, signé, dont l'intégrité se vérifie
   au démarrage sans charger de runtime préalable ;
3. son empreinte au repos doit être **négligeable** (NF-01 : < 150 Mo résidents,
   < 1 % de CPU), sinon les utilisateurs le désinstallent.

## Décision

**Rust**, avec la cible `x86_64-pc-windows-msvc`, pour `ks-broker`, `ks-cli`,
`ks-collectors`, `ks-core` et `ks-agent-linux`.

`unsafe` est `forbid` dans les crates portables (`ks-core`, `ks-cli`,
`ks-agent-linux`), et `warn` là où l'interop Win32 l'impose, chaque bloc portant un
commentaire `// SAFETY:` argumenté.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| **C# / .NET 8 AOT** | Le plus productif sur les API Windows, de loin — WMI, CIM et le registre sont de première classe. Mais même en AOT, la surface d'attaque inclut la réflexion et la désérialisation, et l'empreinte mémoire reste au-dessus de la cible. Surtout : le composant privilégié aurait bénéficié de la sûreté mémoire sans avoir la garantie d'absence de dépendance à un runtime installé. |
| **C++** | Accès natif complet, empreinte minimale — mais aucune sûreté mémoire sur le composant qui en a le plus besoin. Sur un service élevé et permanent, un dépassement de tampon n'est pas un bug, c'est une élévation de privilège. |
| **Go** | Bonne empreinte, compilation statique, mais l'interop Win32 est pénible (cgo ou syscall à la main) et le ramasse-miettes ajoute une variabilité de latence peu souhaitable sur un service surveillé. |
| **PowerShell seul** | C'est la voie la plus rapide pour prototyper, et c'est d'ailleurs pour ça qu'il reste comme *couche d'exécution* appelée par le broker. Mais un service dont le cœur est un interpréteur de scripts viole SEC-02 par construction : il n'y a plus de frontière entre « verbe énuméré » et « exécution arbitraire ». |
| **Rust cible `-gnu` au lieu de `-msvc`** | Permettrait de compiler depuis WSL. Mais ABI différente, bibliothèques d'import différentes, comportements différents sur les structures Win32 : on debuggerait un binaire qui n'est pas celui qu'on livre. Voir `docs/05-ENVIRONNEMENT-DE-DEV.md`. |

## Conséquences

### Ce que ça nous donne

- Sûreté mémoire sur le composant dont la compromission est fatale.
- Binaire natif, sans runtime, facile à signer et à attester (SEC-07, NF-08).
- Empreinte compatible avec les budgets de NF-01.
- Un seul langage du broker à l'agent Linux, et un crate `ks-core` partagé — c'est ce
  qui rend la vue multi-OS possible sans dupliquer la logique.
- `cargo test`, `clippy` et `cargo audit` donnent une base de qualité gratuite.

### Ce que ça nous coûte

- **L'interop Win32 est plus verbeuse qu'en C#.** Chaque collecteur Windows demandera
  plus de lignes que son équivalent PowerShell. C'est accepté, et c'est en partie pour
  ça que PowerShell 7 reste la couche d'exécution pour BitLocker, Defender et les MSU.
- Le vivier de contributeurs est plus étroit.
- La compilation est lente comparée à C# — atténué par un profil `dev` en `opt-level = 0`.

### Ce que ça ferme

Le prototypage rapide dans le broker. Toute idée s'y implémente lentement, ce qui est
un inconvénient réel — et accessoirement une protection : sur un composant privilégié,
la friction décourage les raccourcis.
