# ADR-0014 — Où vivent les valeurs observées, et ce qui compte pour un changement

- **Statut** : Accepté — implémentée le 2026-08-03. Mesuré sur la machine de
  référence : **11 scans consécutifs → 108 lignes** dans `observation`, pas
  1 188 ; **33 lectures refusées** (11 × 3 items illisibles) sans qu'un seul
  intervalle soit fermé ni avancé ; **0 intervalle** sur les chemins
  d'exclusions Defender.
- **Date** : 2026-08-03
- **Exigences concernées** : D2-04, D2-05, SEC-03, SEC-04, P2, P6

## Contexte

Le critère de sortie de la Phase 1 exige « la dérive suivie pendant 7 jours sans
faux positif inexpliqué ». Aujourd'hui, elle n'est mesurable **d'aucune façon** :
le magasin ne stocke que le journal.

### Ce que contient réellement le magasin, mesuré

Relevé le 2026-08-03 sur la machine de référence, par `ks journal` :

```
Journal — 1 entrée(s).
     1  2026-08-02T21:03:55Z  scan  115 item(s)
```

La table `journal` porte `seq`, `at`, `verb`, `target`, `prev_digest`, `digest`,
`payload`. `ks scan --record` y écrit `target: "115 item(s)"` et `diff: None`.
Le journal atteste qu'un scan a eu lieu ; **il ne dit rien de ce qu'il a vu**.
ADR-0011 l'avait nommé ; rien n'a bougé depuis.

### Pourquoi le journal n'est pas le bon endroit

Ce n'est pas une question de place, c'est une **incompatibilité de propriétés**.

| Le journal | Une série d'observations |
|---|---|
| chaîné, chaque entrée ancrée sur la précédente | indépendante, interrogeable par chemin |
| **jamais purgé** — purger casse la chaîne | purgeable, sinon elle croît sans fin |
| lu par un humain, entrée par entrée | lu par une requête, jamais entièrement |
| une entrée par **décision** | 115 valeurs par **sondage** |

Un chaînage et une rétention ne peuvent pas coexister sur la même table :
supprimer un maillon rompt la chaîne, et une chaîne qu'on accepte de rompre ne
prouve plus rien. Verser 115 valeurs par scan dans le journal, c'est choisir de
ne plus jamais rien pouvoir en retirer — à raison d'un scan par heure, 19 320
maillons la première semaine, dont un humain ne lira jamais aucun.

### Ce qui bouge réellement sur cette machine, mesuré

Deux scans consécutifs, le 2026-08-03, à une minute d'intervalle : **un seul
item a changé**, `inventory.uptime_seconds`. Aucune disparition, aucune
apparition.

Sur les sept jours, en revanche, la composition de l'inventaire est brutale :

| Famille | Items | Cadence de changement |
|---|---|---|
| `inventory.software[*].version` | 54 | à chaque mise à jour applicative |
| `inventory.software.*` (agrégats) | 6 | à chaque installation ou désinstallation |
| `security.defender.{signature_version, signatures_applied_at, engine_version}` | 3 | quotidienne, c'est ce qu'on leur demande |
| `virtualization.wsl[*].disk_bytes` | 2 | à chaque usage de la distribution |
| `space.volume[*].used_percent` | 2 | en permanence |
| `security.firewall.inbound_allow_rules` | 1 | à chaque installeur qui pose une règle |
| `inventory.uptime_seconds` | 1 | à chaque scan |
| **Total volatil** | **69 / 115, soit 60 %** | |
| Reste stable | 46, dont **3 illisibles en permanence** et 12 constats de matériel | |

Les 46 stables moins les 12 constats donnent **34 items**, ce qui recoupe
exactement le décompte de déclarables de l'ADR-0009, établi séparément.

Et un piège de forme, mesuré : **26 des 54 chemins logiciels portent leur
version dans le chemin lui-même** — `inventory.software[dbeaver2530currentuser]`,
`inventory.software[postmanx6411795]`, `inventory.software[pgbouncer1241]`. Une
mise à jour de DBeaver ne produit donc pas un changement de valeur : elle produit
une **disparition** et une **apparition**.

### Trois situations que le modèle confond aujourd'hui

Elles se ressemblent, elles n'ont pas le même sens, et les confondre fabrique
exactement le faux positif que le critère de sortie interdit.

1. **La valeur a changé.** `realtime` passe de `true` à `false`. C'est une
   dérive.
2. **On n'a pas su lire.** Les trois exclusions Defender renvoient
   `Illisible { raison }` à chaque scan, faute d'élévation (SEC-01). Un item
   qu'on ne sait plus lire **n'a pas changé** : on ignore s'il a changé.
3. **Le chemin n'est plus produit.** DBeaver désinstallé, ou le collecteur
   logiciel en échec. Le premier est un fait ; le second est une cécité.

`ItemValue::Absent` ne recouvre aucune des trois : c'est un **constat**, « cette
clé n'existe pas », et six items le portent aujourd'hui — dont
`inventory.software[postgisbundle361forpostgresqlx6418removeonly].version`, un
paquet réellement installé dont l'entrée de registre n'expose pas de version.
Écrire `Absent` pour un chemin qui n'a pas été produit fabriquerait une valeur.
C'est la même famille de piège que l'ADR-0010 a fermée entre `Some(Absent)` et
`None`.

### Ce que le code ne sait pas dire aujourd'hui

`Inventory::collect_all` fait `items.extend(c.collect())` et **jette `c.id()`**.
Aucun item ne sait quel collecteur l'a produit. Sans cette information,
« le chemin n'est plus produit » et « le collecteur n'a rien produit » sont
indiscernables — et un échec WMI ferait disparaître d'un coup tous les items
d'état effectif, qu'on prendrait pour la suppression de VBS.

## Décision

**1. Une table de plus, dans la même base.** `observation` rejoint `journal`
dans `%LOCALAPPDATA%\Keystone\journal.sqlite`. Pas de seconde base — un seul
fichier, un seul mode WAL, un seul cycle de vie, une seule chose à sauvegarder.
Pas d'autre format — la valeur se sérialise avec le `ItemValue` existant, dont
l'aller-retour est déjà éprouvé variante par variante.

**2. L'encodage est par intervalles, pas par échantillons.**

```sql
CREATE TABLE observation (
    id         INTEGER PRIMARY KEY,
    path       TEXT    NOT NULL,
    value      TEXT    NOT NULL,   -- JSON d'ItemValue
    first_seen TEXT    NOT NULL,   -- premier scan qui a vu cette valeur
    last_seen  TEXT    NOT NULL,   -- dernier scan qui l'a CONFIRMÉE
    closed     INTEGER NOT NULL DEFAULT 0
);
-- Au plus un intervalle ouvert par chemin. L'invariant est porté par l'index,
-- pas par la vigilance de l'appelant.
CREATE UNIQUE INDEX observation_ouverte ON observation(path) WHERE closed = 0;
CREATE INDEX observation_serie ON observation(path, first_seen);
```

Un scan qui revoit la même valeur **avance `last_seen`** ; il n'insère rien. Un
scan qui voit une autre valeur ferme l'intervalle et en ouvre un.

La taille du magasin devient donc proportionnelle au **changement**, pas au
temps. C'est ce qui rend la décision de rétention triviale : voir le point 6.

Et ce n'est pas une élégance gratuite : l'intervalle *est* la donnée que
l'ADR-0011 réclame. `Change { after_scan_at, before_scan_at }` se lit
directement comme « `last_seen` de l'intervalle fermé » et « `first_seen` du
suivant ». Un sondage ne connaît qu'un intervalle ; le stockage le dit.

**3. Ce qui entre dans la série est décidé par la nature de l'item** (ADR-0009),
et par rien d'autre :

| Nature | Stocké ? | Un changement vaut… |
|---|---|---|
| `Reglage` | oui | une dérive, si l'item est déclaré |
| `Objectif` | oui | une dérive, jamais convergeable |
| `Constat` | oui | un **événement**, jamais une dérive |
| `Mesure` | **non** | rien — hors série en Phase 1 |

Exclure `Mesure` n'est pas un détail de confort : `uptime_seconds` change à
**chaque** scan, donc produirait un intervalle par scan, donc serait à lui seul
la totalité de la croissance du magasin. Les dix items de nature `Mesure` sont
les seuls dont le volume dépende du temps ; les écarter est exactement ce qui
permet le point 6.

**4. Un relevé illisible ne touche à rien.**

Quand `observed` n'est pas un constat (`ItemValue::est_constat()` faux) :

- l'intervalle ouvert **n'est ni fermé, ni avancé** — `last_seen` reste où il
  était, parce qu'on n'a rien confirmé ;
- une ligne est écrite dans `lecture_refusee(scan_id, path, raison)`.

Conséquence, et c'est le point le plus important de cette ADR : quand l'item
redevient lisible avec une autre valeur, l'intervalle de changement s'étend du
**dernier relevé lisible** au **premier relevé lisible suivant**. Il peut couvrir
plusieurs jours. L'attribution de l'ADR-0011, qui exige qu'un événement daté
tombe dans l'intervalle, n'attribuera donc rien, et le changement sortira en
`Provenance::Unknown`.

C'est le résultat **correct** : un changement survenu pendant qu'on était
aveugle est un changement sans auteur identifiable. Et ce n'est pas un faux
positif au sens du critère de sortie, puisqu'il est *expliqué* — la série des
`lecture_refusee` dit exactement quand et pourquoi on n'a pas su regarder.

**5. Une disparition ne se conclut que si le collecteur a parlé.**

`Inventory::collect_all` conserve le producteur de chaque item — le trait
`Collector` expose déjà `id()`, il suffit de cesser de le jeter. Chaque scan
écrit ce que chaque collecteur a produit :

```sql
CREATE TABLE scan            (id INTEGER PRIMARY KEY, at TEXT NOT NULL);
CREATE TABLE scan_collecteur (scan_id INTEGER NOT NULL, collecteur TEXT NOT NULL,
                              items INTEGER NOT NULL, PRIMARY KEY (scan_id, collecteur));
```

Règle : un chemin absent du scan voit son intervalle fermé **si et seulement si**
son collecteur a produit au moins un item lors de ce scan. Sinon, on ne conclut
rien, et l'absence du collecteur est elle-même consignée.

Le producteur ne se **déduit pas** du préfixe du chemin. L'ADR-0009 a déjà
écarté la déduction par préfixe pour la nature, et pour la même raison : elle est
fausse dès la première ligne et elle est silencieuse quand elle se trompe.

**6. Rétention : aucune purge en Phase 1.** Ce n'est pas un renoncement, c'est
une conséquence arithmétique du point 2. Un intervalle pèse de l'ordre de 200
octets. Les 105 items conservés donnent un socle d'environ 21 ko ; une année de
changements réalistes — chaque application mise à jour douze fois, chaque réglage
touché deux fois — ajoute de l'ordre de 700 intervalles, soit 140 ko. Écrire une
politique de purge aujourd'hui reviendrait à configurer avant le deuxième cas
d'usage. Elle se décidera le jour où une mesure la réclame, et la mesure est
facile : `SELECT count(*) FROM observation`.

**7. Le journal ancre le magasin.** La table `observation` **n'est pas
chaînée**, et rien de ce qui suit ne prétend le contraire. Mais `ks scan
--record` écrit déjà une entrée de journal, et son champ `diff` est vide :
il portera désormais l'empreinte BLAKE3 de l'ensemble des observations du scan
— chemins et valeurs, chacun préfixé de sa longueur, comme le fait déjà
`JournalEntry::digest`.

Réécrire l'historique des observations devient donc détectable **contre le
journal**, qui est chaîné, sans que la table d'observations ait à l'être. C'est
le coût d'une empreinte par scan.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Verser les observations dans la table `journal` | chaînage et rétention sont incompatibles : purger romprait la chaîne, ne pas purger la fait croître de 19 320 maillons par semaine. Et un journal que personne ne peut plus lire n'est plus un journal |
| Une seconde base SQLite | deux fichiers, deux WAL, deux ouvertures, deux sauvegardes, deux occasions de désynchronisation — pour une séparation que deux tables donnent déjà |
| Un fichier JSON Lines par jour | pas de requête par chemin sans tout relire ; et le projet a déjà SQLite dans l'arbre pour le journal. Une dépendance de format de plus sans besoin de plus |
| Un échantillon complet par scan (115 lignes × N scans) | croît avec le temps et non avec le changement, donc **impose** une politique de purge dès la première semaine — c'est-à-dire une décision de configuration avant le deuxième cas d'usage |
| Ne stocker que la dernière valeur connue, sans historique | suffit à dire « ça a changé », ne suffit pas à dire **quand**, donc l'attribution de l'ADR-0011 devient impossible : sans intervalle, aucun événement daté ne peut être confronté à quoi que ce soit |
| Écrire `Illisible` comme une valeur de la série | fabrique deux changements — vers l'illisible, puis vers la valeur retrouvée — là où il y en a eu au plus un, à un instant qu'on ignore. C'est le faux positif que `Verdict::Incomparable` a déjà coûté cher à corriger côté comparaison ; le refaire côté stockage annulerait ce travail |
| Écrire `Absent` pour un chemin non produit | confond « la clé n'existe pas », qui est un constat mesuré sur six items, avec « je n'ai pas vu ce chemin ». Même famille que le piège `Some(Absent)` / `None` de l'ADR-0010 |
| Déduire le collecteur d'un item du préfixe de son chemin | écarté pour la nature par l'ADR-0009, faux pour les mêmes raisons ici : `security.platform.vbs_running` vient du collecteur WMI, `security.platform.hvci_policy` du registre, et les deux partagent leur préfixe |
| Chaîner la table d'observations | fait payer à une série interrogeable le prix d'un registre inaltérable, et interdit toute purge future. L'ancrage par le journal (point 7) donne la même détection pour une empreinte par scan |

## Conséquences

### Ce que ça nous donne

La dérive devient mesurable, ce qui débloque le critère de sortie de la Phase 1
et les deux écrans du poste de pilotage qui affichent aujourd'hui « pas encore
calculable ». L'intervalle de changement, qui est la donnée exacte dont
l'ADR-0011 a besoin, tombe gratuitement. Et le magasin reste petit **par
construction**, pas par entretien.

### Ce que ça nous coûte

Une modification de `Inventory` : le producteur de chaque item cesse d'être
jeté. `inv.items` devient une méthode plutôt qu'un champ, ce qui touche deux
sites d'appel (`ks-cli/src/main.rs`, `ui/ks-ui/src/main.rs`).

Une transaction par scan avec 105 mises à jour de `last_seen`. Sur SQLite en
WAL, c'est sous la milliseconde ; le scan lui-même dure 1,3 s, mesuré.

Et une deuxième écriture sur disque à chaque `ks scan --record`. La règle reste
celle du magasin actuel : Keystone écrit dans **son** dossier, jamais dans la
configuration de la machine, et jamais sans qu'on l'ait demandé — `--record`
demeure obligatoire, il n'y a pas d'écriture par omission (P2).

### Ce que ça ferme

Rien. La détection événementielle de D2-04, qui verrait les changements au lieu
de les échantillonner, viendra en Phase 2 avec le broker ; elle alimentera la
même table, avec des intervalles simplement plus courts.

### Ce que ça ne garantit pas

**Un aller-retour entre deux scans est invisible.** Une protection désactivée
puis réactivée dans l'heure ne laisse aucune trace : les deux relevés voient la
même valeur, l'intervalle est simplement confirmé. C'est la limite d'un sondage,
elle est indépassable sans notification du système, et c'est précisément ce que
D2-04 appelle « événementielle quand la source le permet ». En Phase 1, la source
ne le permet pas — le journal Security est refusé sans élévation, mesuré, et
`RegNotifyChangeKeyValue` passe par le crate `windows`, donc par `unsafe`, donc
par une décision non prise.

**Le magasin n'est pas inaltérable.** Il vit dans `%LOCALAPPDATA%`, inscriptible
par l'utilisateur lui-même — l'adversaire A1 du modèle de menace. L'ancrage par
le journal rend la réécriture *détectable*, il ne la rend pas *impossible*. Et il
hérite de la limite déjà écrite dans l'ADR-0004 : la troncature par la fin reste
indétectable localement, quel que soit le nombre d'empreintes. C'est la raison
d'être de SEC-04, Phase 3.

**Un intervalle n'est pas un instant.** Dire « le changement a eu lieu entre
14 h 03 et 15 h 03 » est tout ce que ce magasin sait. Toute présentation qui
afficherait un instant unique mentirait, et fabriquerait dans un journal
forensique une chronologie fausse.

**La cécité ne se distingue pas toujours de la stabilité.** Si un collecteur
échoue à *tous* les scans d'une semaine, ses items gardent un intervalle ouvert
dont le `last_seen` ne bouge plus. Rien ne clignote ; il faut regarder
`scan_collecteur` pour le voir. Un écran qui affiche « conforme » sur un item
dont `last_seen` a une semaine dirait une chose fausse : la fraîcheur de la
confirmation doit remonter jusqu'à l'affichage, comme `unreadable` a dû sortir
du décompte des conformes.
