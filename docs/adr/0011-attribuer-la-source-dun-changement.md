# ADR-0011 — Attribuer la source d'un changement sans élévation

- **Statut** : Accepté — implémentée le 2026-08-17 (`ks_core::Change`,
  `Magasin::changements`, `ks_collectors::attribution`). Voir « Ce que la mise en
  œuvre a mesuré » en fin de document : les chiffres y sont ceux de la machine,
  pas ceux de la décision.
- **Date** : 2026-08-02
- **Exigences concernées** : D2-05, D12-01, D5, SEC-01, P6

## Contexte

D2-05 demande cinq attributions : Windows Update, MDM/Intune, installeur
applicatif, utilisateur horodaté, et **inconnu**. La dernière est le mécanisme le
plus valorisé du modèle : `Provenance::is_security_signal` ne répond vrai que
pour elle, et le projet garde la trace écrite d'un incident où marquer les
relevés `Keystone` l'avait rendue inatteignable, donc morte.

Trois obstacles se dressent, et il vaut mieux les nommer maintenant.

### Premier obstacle : la provenance est portée par une observation

`Item::provenance` décrit un relevé. Sa propre documentation trahit la confusion :
« D'où vient l'information — **ou, pour un changement, qui l'a fait** ». Ce sont
deux questions distinctes. Une observation n'a pas d'auteur ; seul un
**changement** en a un. En Phase 0, tout item collecté vaut `Observed`, et un
test de `ks-collectors` l'impose. C'est juste, et cela signifie que la provenance
d'un changement n'a pas encore de place dans le modèle.

### Deuxième obstacle : aucune valeur observée n'est persistée

Un changement se constate entre deux relevés. Or la table `journal` de
`ks-cli/src/magasin.rs` ne stocke que des `JournalEntry`, et `ks scan --record`
y écrit `target: "115 item(s)"`, `diff: None`. **Les valeurs ne sont nulle part.**
Le journal atteste qu'un scan a eu lieu ; il ne dit pas ce qu'il a vu. En l'état,
la dérive sur sept jours n'est pas mesurable du tout, et cette lacune ne figure
pas dans la liste de la Phase 1.

### Troisième obstacle : ce que la plateforme accorde, mesuré

Relevé sur la machine de référence le 2026-08-02, en session **non élevée**,
c'est-à-dire dans le contexte réel de la CLI (SEC-01) :

| Source | Résultat mesuré | Ce qu'elle donne |
|---|---|---|
| `HKLM\SOFTWARE\Policies\*` | **lisible** | l'autorité, structurellement |
| `HKLM\SOFTWARE\Microsoft\PolicyManager\current\device` | **lisible** | politiques CSP appliquées |
| `HKLM\SOFTWARE\Microsoft\Enrollments` | **lisible** | inscriptions MDM — voir le piège ci-dessous |
| `Win32_QuickFixEngineering` (WMI) | **lisible** | correctifs, avec `InstalledOn` **à la journée près** |
| `Win32_ReliabilityRecords` (WMI) | **lisible**, 572 enregistrements | installations applicatives horodatées |
| `Microsoft.Update.Session` (COM) | **lisible**, 735 entrées | historique Windows Update, horodaté à la seconde |
| Journaux `System`, `Application`, `Setup`, `WindowsUpdateClient/Operational` | **lisibles** | chronologies |
| Journal **`Security`** | **refusé** | *l'événement 4657, « valeur de registre modifiée »* |
| Fichiers `winevt\Logs\*.evtx` en lecture directe | **refusés** | — |
| `Key::last_write_time` dans `windows-registry` 0.6.1 | **n'existe pas** dans l'API | l'instant du dernier changement d'une clé |

Deux conclusions dures.

**On ne saura pas qui a écrit une valeur de registre.** L'événement 4657 est la
seule source qui le dise, il vit dans le journal Security, sa lecture est refusée
sans élévation, et il exige en outre qu'une SACL d'audit soit posée — c'est-à-dire
une écriture système, interdite avant la Phase 2. Aucun choix de bibliothèque n'y
change rien.

**Le journal des événements n'a pas de chemin d'accès sûr en Rust.** Les fichiers
`.evtx` sont verrouillés, et crates.io ne propose aucune enveloppe sûre de
`EvtQuery` : `eventlog` écrit dans le journal, les crates `winevt-*` analysent des
fichiers hors ligne. Y accéder demanderait le crate `windows`, donc `unsafe`, donc
une décision que le projet a délibérément reportée — ou le lancement d'un
processus, qu'un collecteur s'interdit.

### Le piège de la détection MDM

Mesuré sur cette machine, qui n'est **inscrite à aucune MDM** : `Enrollments`
contient **31 sous-clés**, toutes en `EnrollmentState = 1`, dont trois portent un
`ProviderID` (« Local Authority », « Cloud Authority », « Deploy Authority »).
Ce sont les inscriptions CSP intégrées, présentes sur tout Windows 11. Conclure
« `Enrollments` non vide donc géré » produirait un faux positif sur **chaque**
machine, et `Provenance::is_sovereign` deviendrait vrai partout — rendant tout
non convergeable, soit le symétrique exact de l'incident déjà consigné.

## Décision

**1. La provenance d'un changement quitte `Item`.** Un type distinct porte le
changement, avec un **intervalle** et non un instant :

```rust
pub struct Change {
    pub path: String,
    pub before: ItemValue,
    pub after: ItemValue,
    /// Le changement est survenu APRÈS ce relevé…
    pub after_scan_at: Timestamp,
    /// …et AVANT celui-ci. Un sondage ne connaît qu'un intervalle.
    pub before_scan_at: Timestamp,
    pub provenance: Provenance,
}
```

Prétendre à un instant précis fabriquerait, dans un journal forensique, une
chronologie fausse — la faute que `filetime_vers_horodatage` refuse déjà pour les
dates absurdes.

**2. L'attribution se fait par liste blanche, jamais par inférence.** Une source
ne revendique un changement que si **les deux** conditions sont réunies :

- le chemin de l'item figure dans l'ensemble déclaré des chemins que cette source
  est connue pour toucher ;
- l'événement daté de cette source tombe dans l'intervalle du changement.

Sinon : `Unknown`. C'est la règle qui garde le signal vivant. Une corrélation
temporelle large attribuerait, sur une machine qui installe des correctifs chaque
semaine, la quasi-totalité des changements à Windows Update — et `Unknown`
redeviendrait inatteignable.

**3. Trois sources en Phase 1, par ordre de solidité.**

- `Managed(autorité)` — **structurel, pas heuristique.** Une valeur lue sous
  `HKLM\SOFTWARE\Policies\…` ou sous `PolicyManager\current\device` n'a pas
  d'autre auteur possible : c'est ainsi que fonctionne la ruche de politique. Le
  collecteur de posture le sait **déjà** — il distingue
  `security.defender.asr_rules.policy` de `.local` — et jette l'information
  au lieu de la porter jusqu'à la provenance. C'est le gain le moins cher et le
  plus sûr du lot.
- `Keystone` — depuis son propre journal, sur les seules entrées d'application.
  En Phase 1 il n'y en a aucune : la valeur reste inatteignable, et c'est correct.
- `WindowsUpdate` — `Win32_QuickFixEngineering` via le crate `wmi`, déjà dans
  l'arbre et sûr. Restriction assumée : `InstalledOn` est daté **à la journée**,
  ce qui ne permet une attribution que si l'intervalle du changement tient dans
  cette journée. L'historique COM, horodaté à la seconde, exigerait `unsafe` et
  fait l'objet d'une décision séparée.

**4. `Human` et `Application` ne sont pas attribués en Phase 1.** `Human` n'est
connaissable que par le journal des décisions de Keystone (D2-07), donc seulement
pour ce qui passe par l'outil. Tout ce qu'un humain fait en dehors est, en
l'absence du journal Security, honnêtement `Unknown`.

**5. La détection MDM ne repose pas sur la présence d'inscriptions.** Elle exige
un discriminant mesuré sur une machine réellement inscrite. Tant que cette mesure
n'est pas faite, `Provenance::is_sovereign` n'est pilotée que par la ruche de
politique, et D12-01 reste ouverte.

**6. Barrière de test, éprouvée par falsification.** Un changement sur un chemin
absent de toutes les listes blanches doit rendre `Unknown` ; toutes sources
indisponibles, **tout** doit rendre `Unknown`. La barrière se vérifie en injectant
réellement ce qu'elle prétend interdire.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Corrélation temporelle large sans liste blanche | attribue tout à Windows Update sur une machine à jour, et tue le signal `Unknown`. Défaillance déjà survenue sur ce projet, sous une autre forme |
| Lire le journal Security pour l'événement 4657 | **refusé en session non élevée** (mesuré). Exige en outre de poser une SACL, donc une écriture système. Contredit SEC-01 et la règle de la Phase 1 |
| Lire l'heure de dernière écriture des clés de registre | absente de l'API de `windows-registry` 0.6.1 (vérifié sur docs.rs). Passerait par le crate `windows`, donc `unsafe`, décision non prise |
| Lire les `.evtx` avec le crate `evtx` | fichiers **verrouillés** en lecture directe (mesuré). Aucune enveloppe sûre d'`EvtQuery` sur crates.io |
| Attribuer par défaut à `Human` faute de mieux | invente un auteur. C'est l'inverse exact de la doctrine du projet : « un inventaire qui invente est pire qu'un inventaire incomplet » |
| Élever la CLI | SEC-01. Le broker existe pour cela, en Phase 2 |

## Conséquences

### Ce que ça nous donne

Une attribution **vraie là où elle est vraie**, et `Unknown` partout ailleurs —
c'est-à-dire la majorité des cas, ce qui est le résultat correct et non un échec.
Le signal de sécurité de D2-05 reste atteignable, donc vivant. Et une pièce de
Phase 2 posée gratuitement : la ruche de politique nourrit `is_sovereign`, donc
la détection de conflit MDM (D12-03).

### Ce que ça nous coûte

Une table de listes blanches à écrire et à tenir : quels chemins Windows Update
touche réellement. Elle sera incomplète au début, et son incomplétude se voit —
elle produit du `Unknown`, jamais une fausse attribution. C'est le bon sens de
l'erreur.

Et une révision de D2-05 dans le cahier des charges, **dans le même commit** :
l'exigence promet cinq attributions que la plateforme n'accorde pas sans
élévation. Trois sont tenables en Phase 1 ; deux appartiennent au broker. Une
exigence qu'on sait fausse se corrige, elle ne se contourne pas.

### Ce que ça ferme

L'espoir d'une attribution fine du « qui » sans le broker. Cette porte se rouvre
en Phase 2, avec le journal Security et une SACL posée par un verbe, et pas avant.

## Ce que la mise en œuvre a mesuré, le 2026-08-17

### Deux écarts assumés par rapport à l'esquisse

**`Change` est `#[non_exhaustive]`, et son constructeur prend deux couples.**
L'esquisse donnait cinq champs publics, donc cinq paramètres positionnels — dont
deux `ItemValue` et deux `Timestamp`, avec des noms qui se croisent
(`before` va avec `after_scan_at`). C'est un appariement qu'on inverse en
silence. `Change::unattributed(path, (before, after_scan_at), (after,
before_scan_at))` le rend **structurel**, et `#[non_exhaustive]` empêche de
contourner le constructeur par un littéral hors du crate.

**Un changement naît `Unknown`, pas `Observed`.** L'esquisse ne le disait pas.
C'est pourtant la moitié du dispositif : si le repli d'une attribution qui échoue
était `Observed`, `is_security_signal` répondrait faux dès la construction et le
signal serait mort avant d'exister.

### La liste blanche de Windows Update, et sa brièveté

Deux chemins, nommément : `inventory.os.kernel` — le numéro de build change à
chaque cumulatif — et `security.firmware.microcode_revision`. Rien d'autre.

Les versions du moteur et des signatures de Defender **n'y figurent pas**, et ce
n'est pas un oubli : elles sont de nature `Mesure`, donc hors de la série
d'observations (ADR-0014), donc aucun changement n'est jamais construit pour
elles. Les inscrire donnerait une liste qui a l'air plus complète et qui
n'attribuerait rien de plus.

Un test refuse tout chemin `security.defender.*`, `security.services.*`,
`security.firewall.*` ou `security.platform.*` dans cette liste : une protection
ne se met pas à jour par Windows Update, et l'y inscrire rendrait attribuable —
donc invisible — exactement ce que le §6 du modèle de menace place en tête.

### `InstalledOn` : une chaîne, mesurée

La propriété est un `string` de la classe CIM, pas une date. Relevé sur la
machine de référence, en session non élevée : **4 correctifs**, `8/14/2026`,
`8/13/2026`, `8/14/2026`, `10/8/2025`. Les deux `14` et `13` **prouvent** que le
premier nombre est le mois — et que la forme ne suit pas la locale, la machine
étant en `fr-FR`. La seconde forme documentée, un `FILETIME` hexadécimal, est
relue aussi. Toute autre forme rend `None`, et le correctif ne fournit alors
aucune fenêtre : un correctif sans date n'attribue rien, il n'invente pas un jour.

L'absence de fuseau sur `InstalledOn` est une approximation nommée dans le code :
le jour est lu comme un jour UTC. Le garde-fou réel n'est pas la fenêtre, c'est
la liste blanche.

### Les barrières, éprouvées en les franchissant

Trois injections, chacune vérifiée par `grep` avant d'exécuter le test, puis
restaurées :

| Défaut injecté | Ce qui a cassé |
|---|---|
| la liste blanche court-circuitée (`if false`) | `un_changement_hors_liste_blanche_reste_sans_auteur` |
| un changement naissant `Observed` | `un_changement_nait_sans_auteur_et_donc_en_signal` |
| un collecteur rendant `Keystone`, puis `Unknown` | `un_releve_nest_ni_lauteur_de_la_valeur_ni_un_signal` |

La première tentative de falsification a d'ailleurs **échoué en silence** :
`cargo test tests::<nom> -- --exact` filtrait tout, les tests vivant dans des
modules imbriqués (`attribution::tests::…`). C'est le piège que `ci.yml`
documente déjà pour les barrières du broker ; il s'est refermé une fois de plus.

### Ce qui reste sans appelant, et pourquoi

`Magasin::changements` et `attribution::attribuer` sont livrés en bibliothèque,
sans commande `ks` qui les expose. Ajouter un sous-commande à la CLI est un
changement de la surface du produit, et il n'appartient pas à cette décision-ci.
