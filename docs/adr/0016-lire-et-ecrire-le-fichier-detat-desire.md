# ADR-0016 — Lire et écrire le fichier d'état désiré

- **Statut** : Accepté le 2026-08-03 — décisions n° 3 et n° 5 mises en œuvre le
  2026-08-16 (`ks_cli::emetteur` pour l'écriture, `ks_cli::confrontation` pour le
  typage par la forme constatée). La décision n° 7, le schéma généré et relu par
  le lecteur de Keystone en CI, reste à faire.
- **Date** : 2026-08-03
- **Exigences concernées** : D2-01, D2-02, D2-10, P2, P6
- **Précise** : ADR-0010 (dont la décision n° 3 est corrigée sur un point de forme)

## Contexte

L'ADR-0010 a tranché la **forme** du fichier — une section `desired:` indexée
par chemin d'item, l'espace des chemins des collecteurs comme source de vérité,
le schéma JSON généré. Elle a laissé deux choses ouvertes, et les deux bloquent
l'écriture de la moindre ligne de code : **quelle bibliothèque YAML**, et
**comment un scalaire est typé**.

Elle notait déjà qu'« aucun choix n'est confortable ». La mesure du jour le
confirme, et ajoute un piège qu'elle n'avait pas vu.

### Les candidats, mesurés le 2026-08-03

Versions relevées sur l'API de crates.io, MSRV vérifiée **en compilant**, pas en
lisant :

| Crate | Version stable | MSRV déclarée | Résultat à 1.85 | Socle |
|---|---|---|---|---|
| `serde_yaml` | 0.9.34+deprecated | — | — | déprécié, dépôt archivé le 2024-03-25 |
| `serde_yaml_ng` | 0.10.0 | 1.64 | compile | `unsafe-libyaml` (C transpilé) |
| `serde_norway` | 0.9.42 | 1.71.1 | compile | idem, dernière publication déc. 2024 |
| `noyalib` | 0.0.18 | **1.86.0** | refusé par le résolveur | pur Rust, mais version 0.0.x |
| `serde-saphyr` | 1.0.0 | **aucune** | **échoue à la compilation** | pur Rust, `granit-parser` |

`serde-saphyr` 1.0.0 ne déclare aucun `rust-version` et emploie des *let-chains*,
stabilisées en Rust 1.88 et en édition 2024 seulement — vérifié sur les notes de
version 1.88.0. Compilé à 1.85, on obtient cinq fois :

```
error[E0658]: `let` expressions in this position are unstable
   --> serde-saphyr-1.0.0/src/anchors.rs:779:20
```

**C'est la quatrième occurrence du même piège sur ce projet**, après
`libsqlite3-sys` et sa macro `cfg_select!`, `wmi` 0.18 et ses let-chains, et
`noyalib`. Ni le manifeste ni le résolveur ne peuvent l'écarter, faute de
`rust-version` à comparer. Le job MSRV de la CI reste le seul filet, et il ne se
déclenche qu'après avoir compilé.

### Le piège que l'ADR-0010 n'avait pas mesuré : YAML type les scalaires

Vingt et une valeurs réellement relevées ou plausibles, passées à
`serde-saphyr` 1.0.0 par un enum `untagged` `Bool | Int | Text | List`,
mesuré :

| Écrit dans le fichier | Ce qu'on obtient |
|---|---|
| `no` `n` `off` `Off` | `Bool(false)` |
| `yes` `y` `on` `true` `True` | `Bool(true)` |
| `23410000` `26200` | `Int` |
| `0x9` | `Int(9)` |
| `08` | **erreur de désérialisation** |
| `~` `null` | **erreur de désérialisation** |
| `automatique` `P0CN20WW` `25.3.0` `2024-10-21` | `Text` |

Huit scalaires sur vingt et un ne donnent pas ce qu'on croit écrire. Ce n'est
pas propre à ce crate : c'est le typage implicite de YAML 1.1, et le job
`schema` de la CI l'avait déjà nommé en commentaire — « `boost: off` devient
false » — sans que rien, côté runtime, ne s'en protège.

Deux items de la machine de référence sont exposés aujourd'hui :
`inventory.os.kernel` vaut `Text("26200")` et
`security.firmware.microcode_revision` vaut `Text("23410000")`. Déclarés sans
guillemets, ils reviendraient en `Int` et produiraient un écart **permanent** sur
une machine qui n'a jamais changé. Ce sont des `Constat`, donc non déclarables :
la famille est vivante, l'occurrence ne l'est pas encore. Elle le deviendra au
premier réglage dont la valeur ressemble à un nombre ou à `off`.

Et le guillemetage **règle le cas à l'écriture, pas à la lecture** : mesuré,
`"true"`, `"23410000"` et `"08"` reviennent tous en `Text`. Mais le fichier sera
édité à la main pendant sept jours. C'est l'humain qui écrira `off`, et aucune
règle d'écriture ne le protège.

### Deux mesures qui décident du reste

**La clé en double est refusée par défaut.** Mesuré :

```
duplicate mapping key: security.defender.realtime,
set DuplicateKeyPolicy in Options if acceptable
```

C'est une propriété qu'on veut : deux déclarations du même chemin, dont la
seconde l'emporterait en silence, est exactement la manière dont un fichier de
configuration pourrit.

**La syntaxe `!absent` proposée par l'ADR-0010 ne fonctionne pas.** Mesuré :

```
!absent          -> ERREUR  data did not match any variant
{absent: true}   -> Absent
```

Une étiquette YAML personnalisée n'est pas atteignable par un enum `untagged`.
La forme objet, elle, marche — et c'est exactement celle que `ItemValue::Absent`
porte déjà sur le fil depuis sa correction.

## Décision

**1. `serde-saphyr` 1.0.0, en lecture seule.** `default-features = false`,
`features = ["deserialize"]`. Version épinglée à l'exact (`=1.0.0`) : un crate
publié il y a trois jours et sans `rust-version` ne bénéficie pas du bénéfice du
doute d'un `^`.

**2. La MSRV du projet passe de 1.85 à 1.88 — mesuré, et encadré des deux
côtés.** Ce document a d'abord écrit que 1.88 était un *candidat* déduit de la
stabilisation des *let-chains*, en refusant explicitement de l'établir sans
compiler. La mesure a été faite depuis, sur un projet jetable ne dépendant que
de `serde-saphyr` 1.0.0 :

| Toolchain | `cargo check --locked` |
|---|---|
| 1.85.0 | **échec** — `error[E0658]: let expressions in this position are unstable`, cinq fois |
| 1.87.0 | **échec** — même erreur |
| **1.88.0** | **compile**, 9,41 s, sans un avertissement |

Le plancher est donc **exactement 1.88**, et il est encadré : la version juste
en dessous échoue. C'est ce qui distingue une mesure d'une déduction — la
déduction aurait donné le même nombre, sans la certitude qu'aucune autre
contrainte ne se cache au-dessus.

Pour mémoire, c'est la **cinquième** fois que ce projet rencontre une crate
sans `rust-version` déclarée : `libsqlite3-sys` 0.38, `wmi` 0.18, `sysinfo`
0.39, `tauri` par transitivité, et maintenant `serde-saphyr`. Le résolveur 3 en
écarte quatre ; il ne peut rien contre celle-ci, faute de valeur à comparer. Le
job « MSRV » de la CI reste le seul filet.

Relever la MSRV est une décision, et elle
est ici assumée : Keystone est un binaire signé pour Windows 11, pas une
bibliothèque publiée dont d'autres compileraient les sources avec un compilateur
ancien. La toolchain épinglée est déjà 1.97.1.

**3. Keystone n'utilise aucun sérialiseur YAML pour écrire.** `ks import` écrit
le fichier avec un émetteur maison.

> Ce document annonçait « une trentaine de lignes ». Mesuré à la mise en œuvre :
> **130 lignes de code** dans `ks_cli::emetteur`, commentaires et tests exclus.
> L'écart tient à trois choses que l'estimation n'avait pas comptées —
> l'échappement des textes cités, le bloc de liste, et l'en-tête de commentaires
> du fichier — et non à un débordement de portée. Le chiffre est corrigé ici
> plutôt que laissé à croire.

Ce n'est pas une préférence : c'est une nécessité mesurée. Un sérialiseur serde
produit

```yaml
security.services.windefend.startup: automatique
```

sans guillemets, donc un fichier qui se relit correctement **par chance**. Et
aucun sérialiseur serde n'émet de commentaire, alors que l'ADR-0010 promet des
commentaires de section et des clés triées comme contrepartie de la forme plate.
L'émetteur maison :

- trie les clés et insère un commentaire par domaine ;
- **entoure de guillemets tout scalaire textuel**, sans exception ni heuristique ;
- écrit `{ absent: true }` pour un désir d'absence ;
- refuse d'émettre un item dont la valeur constatée n'est pas un constat — on ne
  déclare pas ce qu'on n'a pas su lire, ce qui écarte les trois exclusions
  Defender.

**4. La forme du désir d'absence est `{ absent: true }`, pas `!absent`.**
Correction de la décision n° 3 de l'ADR-0010, sur ce seul point de syntaxe. Le
motif de cette décision — un désir ne peut pas porter `Illisible`, donc `Desire`
ne réutilise pas `ItemValue` — reste entier.

**5. Le scalaire est typé par l'item, pas par YAML.** C'est le point qui
neutralise le typage implicite, y compris pour un fichier écrit à la main.

`desired` se lit d'abord en `BTreeMap<String, ScalaireBrut>`, où `ScalaireBrut`
est ce que YAML a donné. Puis chaque entrée est **contrainte par la forme de la
valeur constatée** de l'item du même chemin :

| Constaté | YAML a donné | Résultat |
|---|---|---|
| `Bool` | `Bool` | accepté |
| `Text` | `Bool(false)` | **refusé** : « la valeur `off` a été lue comme un booléen ; cet item attend un texte. Entourez-la de guillemets. » |
| `Text` | `Int(23410000)` | **refusé**, même message |
| `Int` | `Int` | accepté |
| n'importe lequel | `{absent: true}` | accepté |
| item non observé | — | ni accepté ni refusé : signalé « déclaré, non observé », avec les deux causes possibles (ADR-0010) |

La contrainte se fait dans les deux sens : un item constaté `Bool` déclaré avec
un texte est refusé aussi.

Conséquence assumée : **`ks diff` a besoin d'un scan pour typer le fichier.**
C'est cohérent avec l'ADR-0010 — les collecteurs sont la source de vérité, le
yaml les référence. Un fichier d'état désiré n'a de sens que confronté à une
machine.

**6. Le document se lit avec `deny_unknown_fields` au premier niveau.** Une clé
inattendue à la racine est une faute de frappe ou une version de format plus
récente ; dans les deux cas, l'ignorer en silence est le mauvais choix. Mesuré :
la clé inconnue est bien refusée.

**7. Le schéma JSON est généré depuis les mêmes types**, comme décidé par
l'ADR-0010, et le job `schema` de la CI valide l'exemple contre le schéma
généré. **Le job doit en outre relire l'exemple avec le lecteur de Keystone**,
et pas seulement avec `yaml.safe_load` de Python : deux lecteurs qui valident le
même fichier sont deux sources de vérité, et celle qui tourne en production
n'est pas celle qui garde la CI.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| `serde_norway` 0.9.42, pour rester à MSRV 1.85 | adossé à `unsafe-libyaml`, du C transpilé, dans le chemin qui lit la source de vérité d'un outil de sécurité. Et non maintenu depuis décembre 2024 : on prendrait une dette dont personne ne tient le remboursement |
| `serde_yaml` 0.9.34 | déprécié, dépôt archivé depuis le 2024-03-25 |
| `noyalib` 0.0.18 | version 0.0.x, donc API instable par convention, et MSRV 1.86 : on paierait le relèvement de MSRV **et** l'instabilité |
| Écrire un lecteur de sous-ensemble maison | tentant : le fichier tient en un en-tête, une table plate et une liste. Mais il divergerait du validateur JSON Schema de la CI, qui lit du YAML complet — et deux lecteurs qui ne refusent pas les mêmes fichiers, c'est la dérive silencieuse que ce projet corrige depuis la Phase 0. Reste la porte de sortie si `serde-saphyr` se révèle instable : la surface consommée est deux appels à `from_str` |
| Garder la MSRV à 1.85 et se passer de YAML pour la Phase 1 (TOML, JSON) | D2-01 dit `workstation.yaml`, et le schéma, l'exemple, la CI et le brief l'écrivent tous. Changer de format pour éviter de relever une MSRV serait laisser la queue remuer le chien |
| Utiliser le sérialiseur de `serde-saphyr` pour écrire | mesuré : il écrit les textes sans guillemets, donc un fichier qui se relit juste par chance ; et il n'émet aucun commentaire, ce qui annule la contrepartie promise par l'ADR-0010 à la forme plate |
| Guillemeter à l'écriture et faire confiance à la lecture | protège le fichier que Keystone écrit, pas celui que l'humain édite pendant sept jours. Or c'est l'humain qui écrira `off` |
| Normaliser après coup (`Bool(false)` → `Text("off")` si l'item attend un texte) | devine ce que l'utilisateur voulait dire. `off`, `false`, `no` et `désactivé` deviendraient tous la même chose, alors que le collecteur émet un jeton précis (ADR-0015). Refuser en expliquant coûte une phrase et n'invente rien |
| Accepter la clé en double, dernière gagnante | c'est le mode de pourrissement d'un fichier de configuration : une exception ajoutée en haut, oubliée, écrasée en bas. Le refus est le comportement par défaut du crate ; le désactiver serait un effort pour aller vers le pire |

## Conséquences

### Ce que ça nous donne

Le typage implicite de YAML cesse d'être un risque : il devient une erreur de
lecture, nommée, avec la correction dans le message. Le fichier écrit par
`ks import` est relisible par construction et non par chance. Et la clé en
double, qui est le vecteur classique de pourrissement, est refusée sans effort.

### Ce que ça nous coûte

**Sept crates de plus — mesuré à la mise en œuvre, contre dix annoncés ici.**
`features = ["deserialize"]` tire `num-traits`, `annotate-snippets`,
`granit-parser`, `smallvec` et `encoding_rs_io`, donc aussi `encoding_rs`,
`anstyle`, `unicode-width`, `arraydeque` et `cfg-if`. Mais quatre d'entre elles
— `num-traits`, `smallvec`, `anstyle`, `cfg-if` — étaient **déjà dans l'arbre**.
Le verrou de la racine passe donc de 123 à 130 entrées, et les sept nouvelles
sont : `annotate-snippets`, `arraydeque`, `encoding_rs`, `encoding_rs_io`,
`granit-parser`, `serde-saphyr`, `unicode-width`. Le chiffre de dix était compté
sur la liste des dépendances de la fonctionnalité, pas sur le verrou : c'est la
différence entre lire un manifeste et résoudre un graphe.

C'est beaucoup pour lire une table plate, et `encoding_rs` contient du code
`unsafe` qui entre ainsi dans l'arbre de `ks-cli`, lequel déclare pourtant
`unsafe_code = "forbid"` pour son propre code. Le `forbid` n'a jamais couvert les
dépendances ; ce coût est réel et il est consigné ici pour qu'on cesse de le
découvrir.

**Le verrou de la coque gagne les mêmes sept entrées**, de 465 à 472, et ce
n'était pas prévu : `ks-ui` consomme `ks-cli` par chemin pour son générateur de
rapport, donc l'arbre de lecture du yaml entre chez elle même si elle ne lit
aucun yaml. La dépendance ne va que dans un sens, mais elle transporte tout ce
qui est en dessous.

**Une MSRV relevée**, donc un job de CI à mettre à jour et une promesse à
réécrire dans `Cargo.toml` avec son motif, comme les trois précédentes. **Et
dans `ui/Cargo.toml` aussi**, pour la raison ci-dessus : la coque ne peut pas
promettre moins que ce qu'elle consomme. Mesuré des deux côtés là aussi —
`cargo +1.87.0 check --ignore-rust-version` échoue sur les mêmes let-chains,
`cargo +1.88.0 check` compile.

Le contrôle par `cargo +VERSION` seul ne suffit pas à mesurer un plancher :
dès que `rust-version` est relevé, cargo refuse la compilation **avant** de
compiler (« rustc 1.87.0 is not supported by the following packages »), ce qui
ressemble à un échec de compilation sans en être un. Encadrer le plancher exige
`--ignore-rust-version` sur la version basse, faute de quoi on mesure sa propre
déclaration.

**Un crate de trois jours** dans le chemin qui lit la source de vérité. Épinglé
à l'exact, en lecture seule, et surveillé par `cargo audit` et `cargo deny`
comme les autres. Sa licence, `MIT OR Apache-2.0`, figure déjà dans la liste
blanche de `deny.toml`.

### Ce que ça ferme

Rien d'irréversible. La surface consommée du crate est `from_str` vers deux
types. Le remplacer — par un successeur maintenu, ou par un lecteur de
sous-ensemble maison — reste un travail d'une journée tant que cette surface ne
grandit pas. **Elle ne doit pas grandir** : pas d'ancres, pas d'inclusions, pas
de `figment`, pas de `garde`.

### Ce que ça ne garantit pas

**Le contrôle par la forme constatée ne s'applique qu'aux items observés.** Une
faute de frappe dans un chemin, ou un item légitimement disparu, échappe au
typage puisqu'il n'y a rien à quoi le confronter. `ks diff` les affiche tous deux
comme « déclaré, non observé », en nommant les deux causes — c'est la dette déjà
prise et documentée par l'ADR-0010, et elle n'est pas remboursée ici.

**Le fichier reste sensible à sa propre validité YAML.** Une tabulation, une
indentation irrégulière, un guillemet non fermé donnent une erreur de syntaxe
avant que le moindre contrôle de Keystone s'exécute. Le message viendra du crate,
en anglais, et ne suivra donc pas la règle de voix du projet.

**`deny_unknown_fields` au premier niveau ne dit rien du contenu de `desired`.**
N'importe quelle chaîne y est une clé valide, par construction — il n'existe pas
de catalogue statique des chemins (ADR-0010).

**Rien ici ne protège d'un fichier modifié par un tiers.** `workstation.yaml`
vit dans un dépôt de l'utilisateur, inscriptible par lui — donc par l'adversaire
A1. Keystone lit ce qu'il trouve. Détecter qu'un état désiré a été altéré exige
une signature ou une ancre externe, ce qui n'existe pas avant la Phase 3
(SEC-04).
