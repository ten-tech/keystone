# ADR-0024 — Lire les tâches planifiées sans ouvrir la surface d'écriture de WMI

- **Statut** : Accepté
- **Date** : 2026-08-24
- **Exigences concernées** : D2-03, D5-02, P2, P5, P6, SEC-01

## Contexte

Les tâches planifiées sont citées par D2-03 parmi les domaines couverts, et par
D5-02 parmi les mécanismes de persistance dont toute nouveauté doit être remontée.
Elles ont longtemps été rangées parmi les lectures refusées sans élévation ; la
mesure du 2026-08-24, en session non élevée, dit autre chose.

| Porte | Réponse |
|---|---|
| `HKLM\…\Schedule\TaskCache\Tree` | accès refusé |
| `HKLM\…\Schedule\TaskCache\Tasks` | accès refusé |
| `C:\Windows\System32\Tasks` | accès refusé |
| `MSFT_ScheduledTask`, dans `root\Microsoft\Windows\TaskScheduler` | **194 lignes** |

Trois décisions restaient à prendre, et aucune ne portait sur le privilège.

**La première est technique, et elle est structurante.** Une tâche porte un
tableau `Actions` d'objets de deux classes : `MSFT_TaskExecAction`, qui nomme un
binaire dans une propriété `Execute`, et `MSFT_TaskComHandlerAction`, qui n'en a
pas. Le désérialiseur du crate `wmi` verrouillé demande **chaque champ déclaré**
par `get_property` et propage l'erreur avant que serde voie le champ
(`de/wbem_class_de.rs`, `next_value_seed`). Déclarer `Execute` dans une structure
imbriquée fait donc échouer la requête **entière** dès la première action COM :
mesuré, `0x80041002`, et zéro tâche lue au lieu de 82. `Option<Execute>` n'y
change rien, la propriété étant cherchée avant que son absence soit tolérée.

La voie qu'on prend d'ordinaire dans ce cas — `exec_query` puis `get_property`
objet par objet — fonctionne et coûte le même prix (1,88 à 1,99 s, contre 1,78 à
2,28 s pour la requête typée). Elle a un défaut qui n'est pas de style : elle fait
entrer `IWbemClassWrapper` dans notre code, et ce type expose `put_property`,
`spawn_instance` et `get_method`. Le voisinage aggrave le cas — `PS_ScheduledTask`,
dans **le même espace de noms**, porte `RegisterByXml`, `StartByPath`,
`SetByObject` et `DisableByName`. `RegisterByXml`, c'est `RunCommand` avec un
différé.

**La deuxième est un modèle.** 194 tâches contre 130 items relevés : il n'était
pas question de toutes les publier, et la question « lesquelles, de quelle
nature » est de conception, non de privilège.

**La troisième est un budget.** La classe est lente : 1 777 à 2 279 ms sur
30 lectures consécutives, contre 15 à 320 ms pour `Win32_DeviceGuard` et 275 à
846 ms pour `Win32_Service`. Ni la projection (3 champs ou tous, même durée) ni le
filtrage WQL n'y changent rien : `WHERE NOT TaskPath LIKE '\Microsoft\%'` rend
14 lignes en 1 855 ms, et `WHERE Enabled = TRUE` est refusé. Le fournisseur
énumère tout, puis filtre.

## Décision

**Les tâches se lisent par `query::<T>()`, avec une énumération à balise externe
dont les variantes portent les noms des classes WMI.** Le désérialiseur rend le
nom de classe de l'objet comme identifiant d'énumération (`deserialize_identifier`),
donc une action COM ne se voit demander aucune propriété et une action exécutable
se voit demander `Execute`, et rien de plus. Aucun `IWbemClassWrapper` n'entre
dans le code du dépôt.

**194 tâches deviennent six items du domaine `Configuration`, dont aucun
déclarable**, et ce n'est pas une limite de phase : écrire une tâche demanderait
`RegisterByXml`.

| Item | Valeur mesurée | Nature |
|---|---|---|
| `configuration.tasks.total` | `Int(194)` | Mesure |
| `configuration.tasks.outside_microsoft_root` | `List(14)` | Constat |
| `configuration.tasks.disabled_outside_microsoft_root` | `List(2)` | Constat |
| `configuration.tasks.executables_outside_system_root` | `List(16)` | Constat |
| `configuration.tasks.executables_without_directory` | `List(2)` | Constat |
| `configuration.tasks.com_handler` | `Int(112)` | Mesure |

**Cette lecture porte son propre budget de temps**, quinze secondes, distinct des
cinq secondes de `etat_effectif::DELAI_WMI`.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| `exec_query` + `get_property` par objet | Même résultat, même coût, mais `IWbemClassWrapper` entre dans le code et il porte `put_property`, `spawn_instance` et `get_method`. La lecture seule cesserait d'être portée par la signature pour ne tenir qu'à la discipline. |
| `HashMap<String, Variant>` pour l'action imbriquée | Mesuré : échoue. `deserialize_map` sur un `Variant::Object` retombe sur `deserialize_any`, qui refuse la variante `Object` — « Invalid variant Object(…) ». |
| Énumération `untagged` | Mesuré : échoue, pour la même raison — `untagged` exige `deserialize_any`. |
| Interroger `MSFT_TaskExecAction` directement | Mesuré : **0 instance**. Ces classes ne s'énumèrent pas seules. |
| Filtrer côté WQL pour ne lire que ce qu'on publie | Mesuré : ne gagne rien (1 855 ms pour 14 lignes), et `WHERE Enabled = TRUE` est refusé. |
| Relever `DELAI_WMI` de 5 à 15 s pour tout le monde | Allonge le pire cas des deux lectures qui sont rapides aujourd'hui, pour le confort de celle qui ne l'est pas. |
| Un item par tâche, comme l'inventaire logiciel | **6 des 194 identités portent un identifiant volatil** (SID, GUID, compteur à 18 chiffres) et les 6 sont hors `\Microsoft\`, soit 43 % du lot publié, contre 1 sur 54 côté logiciel. En items séparés, un renommage produirait une disparition **et** une apparition ; dans une liste, il produit un changement. |
| Une liste des **194** identités, qui fermerait les deux angles morts nommés plus bas | Son changement s'afficherait « 194 élément(s) → 195 élément(s) », illisible tant que l'affichage ne sait pas diffuser une liste. Publier ce qu'on ne sait pas lire, c'est P6 à l'envers. Ordre retenu : dépliage d'abord (fait ici), diff de liste ensuite, item de 194 identités en troisième. |
| Ne lister que les tâches **actives** hors `\Microsoft` | Mesuré : 2 des 14 sont à l'arrêt. Les filtrer ferait lire une désactivation comme une **disparition** dans le magasin d'observations, c'est-à-dire exactement la confusion entre « absent » et « à l'arrêt » que le collecteur de posture refuse sur le service `Sense`. La liste porte donc les 14, et une seconde liste nomme les 2. |
| `outside_microsoft_root` nommé `third_party` ou `non_microsoft` | Faux dans les deux sens, et mesuré : `\SoftLanding\…` est une fonctionnalité de Microsoft et vit **hors** de `\Microsoft\`, et rien n'interdit d'enregistrer en dedans. `\Microsoft` est une convention d'emplacement, pas une attestation d'auteur. |
| Publier le chemin du binaire, ses arguments, le SDDL, les CLSID, `Author`, `Triggers` | Chacun mesuré et refusé : un SDDL fait 50 caractères illisibles sans décodeur d'ACL (P6) ; `Author` est vide 71 fois sur 194 et porte des chaînes MUI indirectes (ADR-0011) ; les arguments sont une ligne de commande, qu'on ne veut ni dans le magasin ni dans le rapport ; un CLSID est opaque ; le chemin du binaire porte le nom du compte. |
| Un item « répertoire inscriptible par l'utilisateur » | Serait **faux sur cette machine** : 4 des 16 tâches hors répertoire système lancent `MpCmdRun.exe` depuis `C:\ProgramData\…`, qu'un compte standard ne possède pas. L'emplacement se vérifie ; l'inscriptibilité se calcule, et un calcul faux devient une accusation. |
| Un item pour le croisement « principal privilégié **et** binaire sous un profil utilisateur » | Mesuré : **0**. Le prédicat est une conjonction, donc un composite non dépliable (P6), et bâtir un détecteur pour une forme observée zéro fois est l'abstraction avant le deuxième cas d'usage. Ses deux composantes sont déjà publiées séparément. |
| Un décompte global des tâches désactivées (31) | 29 des 31 sont sous `\Microsoft\` : le nombre mélange la maintenance de l'éditeur et le reste, et ne se déplie donc en rien. C'est la raison pour laquelle `security.firewall` refuse déjà de publier son total de règles. |
| `under_microsoft_root` en plus de `outside` | `total` moins `outside`. Deux sources pour un même fait divergent (ADR-0022). |

## Conséquences

### Ce que ça nous donne

* D2-03 couvre les tâches planifiées, et le domaine `Configuration` publie ses
  premiers items — il en publiait **zéro**.
* D5-02 est tenu par le bon mécanisme : les quatre listes sont des `Constat`, donc
  elles entrent dans le magasin d'observations, et une nouveauté y **ferme un
  intervalle et en ouvre un**. Ce n'est pas le décompte qui porte le signal.
* La lecture seule reste portée par la **signature** : `query::<T>()` ne rend que
  des structures inertes. Une barrière de source
  (`le_collecteur_de_taches_ninvoque_aucune_methode_wmi`) refuse `exec_query`,
  `put_property`, `spawn_instance`, `get_method` et `PS_ScheduledTask`.
* Le dépliage d'une `List` existe enfin dans `ks explain` et dans le rapport HTML.
  Il était dû à P6 depuis les exclusions de Defender : « 2 élément(s) » nommait un
  décompte et jamais ses composantes.

### Ce que ça nous coûte

* **Le scan triple de durée.** Mesuré par retrait plutôt que par calcul : cinq
  `ks scan` avec le collecteur donnent 2 790 à 3 155 ms, les mêmes sans lui
  donnent 934 à 1 086 ms. Ce collecteur est donc à lui seul le poste le plus
  lourd de la collecte, et rien ne le réduit — ni la projection, ni le filtre.
* **Un identifiant de sécurité de compte entre pour la première fois dans une
  valeur d'item.** Mesuré : aucun des 130 items précédents n'en portait, et 5 des
  14 identités hors `\Microsoft` en portent un
  (`\OneDrive Startup Task-S-1-5-21-…`). Le réécrire inventerait une identité que
  Windows n'a jamais enregistrée et rendrait le relevé inutilisable pour retrouver
  la tâche. Un identifiant de sécurité n'est pas un secret ; la distinction est
  écrite ici plutôt que supposée, et elle figure dans la finalité de l'item.
* **Une classe d'action inconnue ferait échouer la requête entière.** Le risque est
  borné — le schéma de cet espace de noms ne contient que deux classes concrètes
  d'action, relevé par `meta_class` — et l'échec est honnête : six items
  illisibles, jamais un décompte faux.
* **La marge du budget de temps n'est pas une garantie.** Sur 36 lectures
  chronométrées, 35 tiennent sous 2,5 s et **une a demandé 5,74 s**. Quinze
  secondes valent 7,5 fois la médiane et 2,6 fois ce pire cas observé. Une
  excursion plus longue publierait six items illisibles pour un scan.

### Ce que ça ferme

* **Aucun de ces six items ne deviendra `Reglage`.** Écrire une tâche, c'est
  `RegisterByXml` ou `SetByObject` : des verbes que la doctrine refuse
  définitivement. Les classer déclarables promettrait une convergence qui
  n'arrivera pas.
* `configuration.tasks.outside_microsoft_root` **pourrait** devenir un `Objectif`
  — figer la liste a la forme de `secure_boot` : on peut le vouloir, aucun verbe
  ne l'écrit. Ce n'est pas fait, et la raison est mesurable : **la stabilité des
  14 identités n'a pas été mesurée**, une session ne le permet pas. La condition
  de promotion est chiffrée : sept scans quotidiens `--record`, et si les
  14 identités tiennent, une ADR ouvre la porte.

## Les deux angles morts, nommés plutôt que tus

Les items 1 et 6 sont des `Mesure`, donc non stockés. La couverture exacte de la
détection de nouveauté est donc celle-ci, et pas davantage :

| Nouveauté | Vue par |
|---|---|
| tâche hors racine `\Microsoft` | `outside_microsoft_root` |
| tâche, n'importe où, lançant un binaire hors du répertoire système | `executables_outside_system_root` |
| tâche, n'importe où, lançant un binaire sans répertoire | `executables_without_directory` |
| tâche sous `\Microsoft\` lançant un binaire du répertoire système | **rien** |
| tâche sous `\Microsoft\` passant par un gestionnaire COM | **rien** |

Ce qui fermerait les deux dernières lignes est une `List` `Constat` des 194
identités. Elle est écartée ci-dessus, et l'ordre de travail y est écrit.

## Ce qui n'a pas été mesuré

* **Le dépôt WMI à froid.** Toutes les durées sont à chaud.
* **La stabilité des 14 identités dans le temps** — ce qui bloque la promotion en
  `Objectif`.
* **`Settings`**, l'objet imbriqué de configuration d'une tâche, n'a pas été
  ouvert : aucun item proposé n'en a besoin.
* **Un dossier du planificateur écrit en casse différente** (`\microsoft\`). La
  comparaison est insensible à la casse, au motif que Windows y voit le même
  dossier ; le vérifier exigerait de **créer** un dossier, c'est-à-dire d'écrire.
* **La partition 82 / 112 sans recouvrement** est un fait de cette machine. Rien
  n'interdit à une tâche de mêler les deux classes d'action, et le code n'en
  dépend pas.
