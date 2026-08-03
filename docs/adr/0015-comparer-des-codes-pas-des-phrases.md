# ADR-0015 — Comparer des codes, pas des phrases

- **Statut** : Accepté — implémentée le 2026-08-03 (`ks_collectors::jetons`,
  libellés dans `ks_cli::lisible`). Mesuré sur la machine de référence :
  **116 items, dont 35 déclarables** — 28 `Reglage` et 7 `Objectif`, contre
  14 `Mesure` et 67 `Constat`. Trois écarts entre ce qui suit et ce qui a été
  livré sont consignés en fin de document, section « Ce que l'implémentation a
  corrigé de cette décision ».
- **Date** : 2026-08-03
- **Exigences concernées** : D2-01, D2-02, D2-04, P6, NF-05

## Contexte

`Item::verdict()` compare `desired` et `observed` par **égalité de
`ItemValue`**. Pour douze items relevés sur la machine de référence, cette
égalité porte sur une **phrase française rédigée par le collecteur**.

### Ce qui est réellement comparé, mesuré

Relevé le 2026-08-03, sur les 115 items :

| Chemin | Valeur constatée |
|---|---|
| `security.platform.lsa_protection` | `activée, sans verrou UEFI` |
| `security.platform.vbs_running` | `en cours d'exécution` |
| `security.platform.hvci_running` | `en cours d'exécution` |
| `security.platform.credential_guard_running` | `à l'arrêt` |
| `security.platform.code_integrity_enforcement` | `imposée` |
| `security.platform.dma_protection_available` | `disponible sur ce matériel` |
| `security.services.{windefend,mpssvc,eventlog,wscsvc,bits}.startup` | `automatique` |
| `inventory.software.attribution` | `complète` |

Ces douze items ne sont pas un échantillon quelconque : ce sont **presque tous
des déclarables** au sens de l'ADR-0009, c'est-à-dire précisément ceux qui
entreront dans `workstation.yaml` et dont la comparaison décidera de la posture
affichée.

Le code qui les produit, dans `ks-collectors/src/posture.rs` :

```rust
Lecture::Trouvee(0) => "désactivée".to_owned(),
Lecture::Trouvee(1) => "activée, verrouillée par UEFI".to_owned(),
Lecture::Trouvee(2) => "activée, sans verrou UEFI".to_owned(),
Lecture::Trouvee(n) => format!("code inconnu ({n})"),
```

### Trois conséquences, dont une rétroactive

**Le fichier d'état désiré contient de la prose.** L'utilisateur devra écrire,
à la main, dans un fichier de configuration :

```yaml
security.platform.lsa_protection: "activée, sans verrou UEFI"
```

Avec la virgule, avec les accents, au mot près. Une configuration qu'on ne peut
pas écrire de mémoire est une configuration qu'on copie-colle, donc qu'on ne
relit pas — soit exactement ce que l'ADR-0009 cherche à éviter en limitant le
fichier à trente lignes.

**Un changement de libellé est un changement de format.** Corriger une coquille,
retirer une virgule, remplacer « à l'arrêt » par « arrêté » : chacun de ces
gestes fait basculer d'un coup **tous** les items déclarés de ce collecteur en
`Verdict::Ecart`, sur une machine où rien n'a bougé. Le compilateur ne dit rien,
aucun test actuel ne dit rien, la revue voit une amélioration de style.

Et l'effet est **rétroactif** : le magasin d'observations de l'ADR-0014 conserve
les valeurs telles qu'écrites. Une reformulation transforme donc la série des
sept jours passés en une série où tout a changé le jour de la recompilation. Le
critère de sortie de la Phase 1 ne serait pas seulement raté ; il serait
invalidé après coup, sans qu'aucune donnée n'ait été touchée.

**Une valeur est un composite non déplié.** `activée, sans verrou UEFI` porte
**deux** faits dans une seule chaîne : la protection est active, et son verrou
UEFI est absent. Le principe P6 exige qu'un indicateur composite soit toujours
dépliable en ses composantes exactes. Il ne l'est pas, et on ne peut donc pas
déclarer « je veux la protection LSA active » sans se prononcer, dans la même
chaîne, sur le verrou.

### La règle existe déjà, appliquée ailleurs, écrite noir sur blanc

Ce n'est pas une doctrine à inventer. Elle est déjà posée deux fois dans ce
dépôt, et deux fois pour la même raison.

Dans `ks-cli/src/lisible.rs` :

> Un item porte des **octets**, parce qu'un octet est ce que la machine a mesuré
> et que la Phase 1 comparera des octets. […] La conversion appartient donc à
> l'affichage, jamais au relevé.

Dans `ks-core/src/journal.rs`, sur `Actor::etiquette_stable` :

> Surtout pas `{:?}` […] Une empreinte adossée à `Debug` rend invérifiable,
> après une simple montée de compilateur, un journal déjà expédié hors machine.
> […] Ces valeurs **ne se renomment jamais** ; elles font partie du format.

`virtualization.wsl[debian].disk_bytes` vaut `56143904768` dans le relevé et
« 52,3 Gio » à l'écran. `Actor::Human` vaut `"human"` dans l'empreinte et
« Humain » à l'écran. `security.platform.lsa_protection` vaut « activée, sans
verrou UEFI » dans les deux. La règle est écrite ; elle n'a pas été appliquée là
où le collecteur de posture traduit des codes de registre.

## Décision

**1. Toute valeur qui sort d'une table de codes appartient à un vocabulaire
fermé, stable et testé.** Les chaînes émises sont des **jetons**, pas des
phrases : minuscules, sans accent, sans ponctuation, sans espace.

```
lsa_protection          →  active-verrou-uefi | active | inactive | code-inconnu:<n>
vbs_running             →  en-execution | configure-non-demarre | eteint | code-inconnu:<n>
code_integrity          →  imposee | audit | eteinte | code-inconnu:<n>
services.*.startup      →  automatique | automatique-differe | manuel | desactive | code-inconnu:<n>
software.attribution    →  complete | partielle
```

Le suffixe `code-inconnu:<n>` conserve la nuance à laquelle
`demarrage_service` tient déjà : un octet parasite se **nomme**, il ne tombe
pas dans « désactivé ».

> Deux lignes de cette table ont été corrigées à la mise en œuvre — celle des
> services, incomplète, et le jeton `active`, qui aurait servi deux
> vocabulaires. La table réellement livrée est celle de
> `ks_collectors::jetons` ; les écarts et leur raison sont en fin de document.

**2. Ces jetons sont un format, au même titre que `Actor::etiquette_stable`.**
Ils ne se renomment jamais. Un test les fige, listé à la main mais gardé par un
`match` exhaustif sans bras `_` sur la table de codes : ajouter un code casse la
compilation avant qu'un test s'exécute.

**3. Le libellé français vit dans `ks-cli::lisible`, avec la conversion
d'octets.** Un seul module de présentation, partagé par la CLI et la coque —
c'est déjà la raison d'être de ce module, et c'est déjà pourquoi il vit dans la
bibliothèque et non dans le binaire. `ks explain`, `ks scan`, le rapport HTML et
le poste de pilotage affichent donc « activée, sans verrou UEFI » exactement
comme aujourd'hui. **Rien ne change à l'écran** — au dépliage du point 4 près,
qui déplace la mention du verrou dans son propre item (voir la fin du
document).

**4. Un composite se déplie en items.** `security.platform.lsa_protection` donne
deux items :

```
security.platform.lsa_protection            active | inactive | code-inconnu:<n>
security.platform.lsa_protection_uefi_lock  true | false
```

C'est ce que P6 demande, et cela rend déclarable « je veux la protection LSA
active » sans obliger à se prononcer sur le verrou. Le second item est un
`Objectif` au sens de l'ADR-0009 : on peut le vouloir, aucun verbe ne l'écrit
seul.

**5. La règle s'applique au point de construction, comme la nature.** Le
collecteur émet le jeton ; il n'existe aucun endroit où une phrase française
entre dans un `ItemValue`. Le contrôle est mécanique : un test parcourt
`Inventory::collect_all()` et refuse tout `ItemValue::Text` d'un item de nature
`Reglage` ou `Objectif` qui contienne un espace, une majuscule ou un caractère
non ASCII. Les `Constat` en sont exemptés — `inventory.os.name` vaut
« Windows 11 Home » et c'est la valeur exacte donnée par le système, pas une
traduction que nous aurions choisie.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Figer les phrases actuelles par un test, sans les changer | ferme la régression, mais laisse la prose dans le fichier de configuration et laisse le composite non déplié. On paierait le test sans obtenir ni la lisibilité ni P6 |
| Comparer en ignorant la casse, les accents et la ponctuation | une normalisation est une devinette : « activée, sans verrou UEFI » et « activée sans verrou UEFI » deviendraient égales, mais « arrêté » et « à l'arrêt » resteraient différentes. On aurait la fragilité, plus une illusion de robustesse |
| Ajouter un champ `label` à côté de `value` dans `ItemValue` | fait porter la présentation par le modèle, alors que le projet a déjà tranché l'inverse pour les octets. Et double la surface sérialisée de chaque item, pour une information que `lisible` sait recalculer |
| Garder l'entier brut du registre comme valeur (`2` pour `lsa_protection`) | `ks explain` afficherait `2`, et le fichier d'état désiré aussi. On perd l'explicabilité de P6 au lieu de la gagner, et on rend le yaml illisible autrement qu'avec la documentation de Microsoft à côté |
| Reporter la correction en Phase 2, quand la convergence écrira ces valeurs | trop tard de sept jours : la série d'observations de la Phase 1 aura été enregistrée en prose, et sa reformulation l'invalidera rétroactivement. Le coût de la correction augmente avec la durée de l'historique |

## Conséquences

### Ce que ça nous donne

Un `workstation.yaml` qu'on écrit de mémoire. Une comparaison qui survit à une
relecture de style. Un historique d'observations qui reste valable après une
recompilation. Et un composite de moins, donc une violation de P6 réparée en
chemin.

### Ce que ça nous coûte

Environ douze sites d'émission dans `posture.rs` et `etat_effectif.rs`, plus la
table de correspondance jeton → libellé dans `lisible.rs`, plus le dépliage de
`lsa_protection` en deux items — soit un item de plus dans l'inventaire, qui
passe de 115 à 116, et deux documents à corriger dans le même commit
(`07-FEUILLE-DE-ROUTE.md`, `03-ARCHITECTURE.md`, qui citent le décompte).

Et une double lecture pour qui débogue : le jeton dans le JSON, le libellé à
l'écran. C'est le prix déjà payé pour les octets, et il est jugé bon.

### Ce que ça ferme

L'idée qu'un `ItemValue::Text` puisse être écrit pour être lu par un humain. Il
est écrit pour être **comparé** ; ce qui est écrit pour être lu vit dans
`lisible`.

### Ce que ça ne garantit pas

**Les valeurs qui ne sortent pas d'une table de codes restent de la donnée
brute, et elles bougent.** `security.firmware.version` vaut `P0CN20WW`,
`security.clock.ntp_server` vaut `time.windows.com,0x9`, les 54 versions
logicielles valent ce que le registre contient. Aucun vocabulaire ne les
stabilise, et ce n'est pas le rôle de cette décision : ce sont des `Constat`,
ils ne se déclarent pas.

**Un jeton figé ne protège pas d'un changement de sens.** Si Microsoft ajoute un
code `3` à `RunAsPPL`, `code-inconnu:3` apparaîtra et le jeton précédent gardera
la même écriture pour un état devenu ambigu. Le test fige l'orthographe, pas la
sémantique de la plateforme.

**Le contrôle mécanique du point 5 est un filet, pas une preuve.** Il refuse
l'espace, la majuscule et l'accent ; il ne refuse pas un jeton mal choisi, ni
deux codes distincts traduits par le même jeton. C'est la revue qui l'attrape,
comme pour la barrière SEC-02 — aucun test grossier ne remplace la lecture.

**Les modes des règles ASR restent en français.** `mode_asr` est bien une table
de codes, mais sa sortie vit à l'intérieur d'un `ItemValue::List`, accolée au
GUID de la règle (`<guid> = audit`), et non dans un `Text`. Le contrôle du point
5 ne la voit donc pas. La décider suppose de trancher ce qu'est le jeton d'une
*liste de couples*, ce que cette décision-ci ne fait pas : la limite est nommée
en tête de `ks_collectors::jetons` plutôt que laissée à la découverte.

## Ce que l'implémentation a corrigé de cette décision

Trois affirmations ci-dessus se sont révélées fausses à la compilation. Elles
sont corrigées ici plutôt que réécrites plus haut : une décision se lit avec ce
que sa mise en œuvre lui a appris.

**Le type de démarrage porte cinq codes, pas trois.** La table annoncée
(`automatique | automatique-differe | manuel | desactive`) oubliait les codes
`0` et `1`, que Microsoft documente et que le collecteur traduisait déjà — les
y laisser tomber en `code-inconnu` aurait perdu une information réelle. Et
`automatique-differe` ne correspond à aucun octet de `Start` : le démarrage
différé vit dans `DelayedAutostart`, que rien ne lit aujourd'hui. Le jeton aurait
donc désigné un état qu'aucun code n'émet. La table livrée est
`demarrage-noyau | demarrage-systeme | automatique | manuel | desactive |
code-inconnu:<n>`.

**Le jeton `active` ne peut pas servir deux vocabulaires.** Le point 4 déplie la
protection LSA, et son `active` recouvre alors les codes 1 et 2. Credential
Guard, lui, n'est pas déplié et garde le composite. Deux items donneraient donc
au même jeton deux sens distincts, ce que la section précédente nomme
elle-même comme le défaut qu'aucun test n'attrape. Le composite s'écrit donc
`active-sans-verrou-uefi`, et `active` reste réservé à l'état déplié.

**« Rien ne change à l'écran » vaut pour onze items sur douze.** Le déménagement
du libellé est bien invisible : `automatique`, `en cours d'exécution`,
`à l'arrêt`, `imposée`, `disponible sur ce matériel` et `complète` s'affichent
au caractère près comme avant, dans `ks scan`, `ks explain`, le rapport HTML et
le poste de pilotage. Mais le point 4 change nécessairement le douzième :
`security.platform.lsa_protection` affiche désormais « activée », et la mention
du verrou vit dans l'item voisin `security.platform.lsa_protection_uefi_lock`.
C'est le prix du dépliage, et c'est le principe P6 qui l'exige — un indicateur
composite n'a pas le droit de rester composite parce que son libellé est
joli. La phrase du point 3 était vraie du déménagement, pas du dépliage.

**`security.clock.ntp_server` et `security.clock.timezone` ne sont pas des
`Constat`.** La section « Ce que ça ne garantit pas » l'affirmait ; ce sont des
`Reglage`, donc déclarables, donc soumis au contrôle du point 5 — que
« Romance Standard Time » ne passe pas. Leur valeur reste néanmoins une donnée
brute recopiée de Windows, pas une traduction que nous aurions choisie : les
réduire à des jetons reviendrait à inventer un vocabulaire par-dessus celui de Microsoft.
Le contrôle porte donc une exemption **nommée, courte et gardée** — chaque
chemin cité doit exister et rester déclarable, sinon le test échoue.
