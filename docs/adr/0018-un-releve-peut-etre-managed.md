# ADR-0018 — Un relevé peut être `Managed`, jamais `Keystone` ni `Unknown`

- **Statut** : Accepté — implémentée le 2026-08-17 (`ks_collectors::politique`,
  test `un_releve_nest_ni_lauteur_de_la_valeur_ni_un_signal`). Mesuré sur la
  machine de référence : **zéro item `Managed`**, et c'est un résultat, pas un
  échec — voir « Ce que la mise en œuvre a mesuré » en fin de document.
- **Date** : 2026-08-03
- **Exigences concernées** : D2-05, D12-01, D12-03, P10, SEC-01
- **Complète** : ADR-0011 (dont elle ferme le seul point resté ouvert côté code)

## Contexte

L'ADR-0011 a tranché l'essentiel de l'attribution : le changement porte sa
provenance dans un type distinct, l'attribution se fait par liste blanche et
jamais par inférence, trois sources en Phase 1, `Human` et `Application`
reportés au broker. Elle a mesuré, en session non élevée, ce que la plateforme
accorde et ce qu'elle refuse.

Il en reste **un** point qui n'est pas une conséquence des autres, et qui ne peut
pas être écrit sans décision, parce qu'il touche un test posé après un incident.

### Le test qui barre la route, et pourquoi il existe

`ks-collectors/src/lib.rs` :

```rust
#[test]
fn un_releve_ne_sattribue_pas_la_paternite_de_la_valeur() {
    for item in Inventory::collect_all().items {
        assert_eq!(item.provenance, Provenance::Observed, …);
        assert!(!item.provenance.is_security_signal(), …);
        assert!(!item.provenance.is_sovereign(), …);
    }
}
```

Son commentaire dit exactement ce qu'il garde :

> Marquer cela `Keystone` était faux, et avait une conséquence lourde —
> `Unknown` devenait inatteignable, donc `is_security_signal` toujours faux,
> donc le signal le plus valorisé du modèle structurellement mort en Phase 0.

Mesuré le 2026-08-03 : les **115 items** portent `Provenance::Observed`. Le test
tient, et il a raison de tenir.

Mais il tient **trois** choses en une seule assertion d'égalité, et deux
seulement méritent d'être tenues.

### Ce que la ruche de politique accorde, structurellement

L'ADR-0011 l'a mesuré : `HKLM\SOFTWARE\Policies\*` et
`HKLM\SOFTWARE\Microsoft\PolicyManager\current\device` sont **lisibles** sans
élévation.

Et ce n'est pas une heuristique. Une valeur lue sous la ruche de politique n'a
pas d'autre auteur possible : c'est ainsi que fonctionne la ruche. Le collecteur
de posture le sait **déjà**, et le dit dans ses chemins — mesuré ce jour :

```
security.defender.asr_rules.policy   {absent: true}
security.defender.asr_rules.local    []
```

Deux chemins, deux origines, une seule provenance : `Observed` pour les deux.
L'information est produite, puis jetée au moment précis où elle servirait.

### Ce que ça coûte de la jeter

`Provenance::is_sovereign` n'est vrai que pour `Managed(_)`, et il n'est vrai
nulle part aujourd'hui. Donc :

- `DriftStatus::Conflict`, qui existe dans le modèle et qui est testé, n'est
  **jamais construit** ;
- P10, « la MDM est souveraine », n'a aucun support dans les données ;
- D12-03, la détection de conflit de politique gérée, n'a pas de première pierre.

C'est le symétrique exact de l'incident consigné : là on avait rendu `Unknown`
inatteignable, ici on maintient `Managed` inatteignable. Dans les deux cas, une
valeur du modèle ne peut pas être produite, donc le mécanisme qu'elle porte est
mort.

### La confusion qu'il faut lever

`Provenance` répond aujourd'hui à deux questions, et sa propre documentation
l'avoue : « D'où vient l'information — **ou, pour un changement, qui l'a fait** ».
L'ADR-0011 sépare les deux en sortant la provenance d'un changement vers
`Change`. Une fois cette séparation faite, `Item::provenance` ne répond plus
qu'à **une** question : *d'où vient cette lecture ?*

Et à cette question, « d'une ruche de politique » est une réponse vraie,
vérifiable, et sans prétention sur l'auteur du changement.

## Décision

**1. `Item::provenance` vaut `Managed(autorité)` pour une valeur lue sous une
ruche de politique, et `Observed` partout ailleurs.** Aucune autre valeur n'est
atteignable depuis un collecteur.

**2. L'autorité nommée est celle qu'on connaît, pas celle qu'on suppose.** Sans
discriminant MDM mesuré sur une machine réellement inscrite — l'ADR-0011 a
montré que 31 sous-clés d'`Enrollments` existent sur une machine qui n'est
inscrite nulle part —, l'autorité est la ruche elle-même :

```
Managed("stratégie de groupe ou MDM")
```

Pas `Managed("Intune")`. Nommer une autorité qu'on n'a pas identifiée serait
inventer, ce que la doctrine du projet refuse depuis l'inventaire logiciel.
D12-01 reste ouverte, comme l'ADR-0011 l'a acté.

**3. Le test change de forme, pas de sévérité.** L'égalité à `Observed` est
remplacée par les deux interdictions qu'elle portait réellement, plus un
encadrement de la nouvelle valeur :

```rust
// Ce que le collecteur ne peut JAMAIS produire, et pourquoi.
// - Keystone : l'outil n'est l'auteur d'aucune valeur qu'il a lue. C'est la
//   faute déjà commise, qui rendait `Unknown` inatteignable.
// - Unknown  : un RELEVÉ n'est pas un CHANGEMENT. Le signal de sécurité de
//   D2-05 naît d'une comparaison entre deux relevés, jamais d'un seul.
assert_ne!(item.provenance, Provenance::Keystone);
assert!(!item.provenance.is_security_signal());

// Et `Managed` n'est pas un fourre-tout : il ne se produit que là où la
// lecture vient réellement d'une ruche de politique.
match &item.provenance {
    Provenance::Observed => {}
    Provenance::Managed(_) => assert!(chemins_de_politique().contains(&item.path)),
    autre => panic!("« {} » : provenance interdite pour un relevé — {autre:?}", item.path),
}
```

Le `match` est **exhaustif sans bras `_`** : ajouter une variante à `Provenance`
casse la compilation, donc oblige à décider si un collecteur a le droit de la
produire.

**4. La barrière s'éprouve par falsification.** Un collecteur fabriqué qui rend
`Managed` sur un chemin hors ruche doit faire échouer le test ; on l'injecte
réellement, on vérifie que ça casse, on restaure. Une barrière qu'on n'a pas
essayé de franchir ne prouve rien.

**5. `is_sovereign` ne déclenche rien en Phase 1.** Elle devient simplement
atteignable, et le poste de pilotage peut afficher « imposé par une politique »
à côté d'un item. Le refus de convergence qu'elle commandera (P10) appartient à
la Phase 2, où il y a quelque chose à refuser.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Laisser tous les relevés en `Observed` et déduire la souveraineté du chemin au moment d'afficher | seconde source de vérité : le collecteur sait, l'afficheur redevine. L'ADR-0009 a écarté la déduction par préfixe pour la nature, exactement pour cette raison, et ici le préfixe ne suffit même pas — `asr_rules.policy` et `asr_rules.local` partagent tout sauf le dernier segment |
| Marquer `Managed` dès qu'`Enrollments` n'est pas vide | mesuré faux sur cette machine : 31 sous-clés, aucune inscription réelle. `is_sovereign` deviendrait vrai partout, donc tout non convergeable — le symétrique exact de l'incident déjà consigné |
| Nommer l'autorité `Intune` par défaut | invente. Et une fausse attribution est pire qu'une absence d'attribution : elle ne se corrige jamais, parce que personne ne la met en doute |
| Attendre la Phase 2 et le broker | la ruche de politique est **lisible sans élévation**, mesuré. Rien n'est gagné à attendre, et sept jours d'observations seraient enregistrés sans cette information |
| Ajouter un champ `source_de_lecture` à `Item` plutôt que réutiliser `provenance` | `provenance` répond déjà à « d'où vient cette lecture » une fois que l'ADR-0011 en a sorti la provenance d'un changement. Deux champs pour une question sont un champ de trop |
| Garder l'assertion `== Observed` et ne rien changer | maintient `Managed` inatteignable, donc `DriftStatus::Conflict` inconstructible, donc P10 sans support. C'est la situation actuelle, et c'est un mécanisme mort qu'on croit vivant parce qu'il est testé |

## Conséquences

### Ce que ça nous donne

`Provenance::Managed` devient atteignable, donc `is_sovereign` devient utile,
donc P10 et D12-03 ont leur première pierre — posée gratuitement, à partir d'une
information que le collecteur produisait déjà et jetait. Et `Item::provenance`
ne répond plus qu'à une seule question, ce qui est la condition pour que
`Change::provenance` de l'ADR-0011 réponde à l'autre.

### Ce que ça nous coûte

Un test réécrit, et il faut le réécrire **bien** : il garde la trace d'un
incident, et une réécriture négligente le transformerait en test qui passe
toujours. La liste des chemins de politique doit vivre dans le collecteur qui
les lit, pas dans le test — sinon c'est le test qui devient la source de vérité,
et il dérivera.

Et un affichage de plus à concevoir : un item imposé par une politique ne se
présente pas comme un item ordinaire, sous peine de laisser croire qu'on pourra
le changer.

### Ce que ça ferme

L'idée qu'un relevé n'ait qu'une seule provenance possible. Elle était commode
et elle était fausse : la ruche de politique est une origine, pas une supposition.

### Ce que ça ne garantit pas

**On ne sait toujours pas *qui* a écrit la politique.** `Managed` dit qu'une
autorité impose la valeur ; il ne dit pas si c'est Intune, une GPO de domaine,
ou un `reg add` de l'utilisateur lui-même sous `HKLM\SOFTWARE\Policies`. Ce
dernier cas est réel sur un poste personnel, et il produit un faux `Managed`.
L'erreur va dans le sens sûr — Keystone refusera d'écrire là où il croit une
autorité présente — mais elle reste une erreur, et elle rendra un item non
convergeable sans raison en Phase 2.

**`Unknown` reste inatteignable tant que le magasin de l'ADR-0014 n'existe
pas.** C'est correct : `Unknown` qualifie un changement, et il n'y a pas de
changement sans deux relevés comparés. Mais cela signifie que le mécanisme le
plus valorisé du modèle reste mort jusqu'au lot qui livre le magasin, et il faut
le savoir plutôt que de le découvrir en cherchant pourquoi rien ne remonte.

**Rien ici ne détecte l'oscillation** (D12-04), qui exige de voir une valeur
repoussée à chaque cycle — donc l'historique du magasin, donc la Phase 1 finie,
donc au plus tôt la Phase 2.

## Ce que la mise en œuvre a mesuré, le 2026-08-17

### Zéro item `Managed`, et pourquoi c'est un résultat

Sur les **120 items** de la machine de référence, aucun ne ressort `Managed`.
La raison est mesurée trois fois, par trois chemins indépendants :

- `Test-Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows Defender\Windows Defender Exploit Guard\ASR\Rules'` répond **False** ;
- l'énumération de `HKLM\SOFTWARE\Policies\Microsoft\Windows Defender` ne montre **ni valeur, ni sous-clé** ;
- `ks scan --json` publie `security.defender.asr_rules.policy = {"absent": true}`, contre `security.defender.asr_rules.local = []` — la clé locale existe, celle de stratégie non.

C'est la conclusion attendue sur un poste personnel non inscrit, et c'est
exactement ce que la décision prévoyait : `Managed` devient **atteignable**, il
ne devient pas *fréquent*. Une machine réellement gérée le produira.

### `Managed` exige qu'une valeur ait été **lue**

Point que la décision laissait implicite, et qui compte autant que le reste :
une clé de politique **absente** ou **refusée** ne marque rien. Marquer une
absence rendrait `is_sovereign` vrai sur *toute* machine — donc tout non
convergeable — c'est-à-dire le piège d'`Enrollments` de l'ADR-0011, remonté d'un
étage. Une liste **vide**, elle, compte comme lue : la clé existe sous la ruche,
une autorité l'a créée, et « cette autorité n'impose aucune règle » est un fait.

### Le `match` de la décision n° 3 n'était pas exhaustif

L'esquisse écrivait `autre => panic!(…)`. C'est un `_` déguisé : un bras de
liaison capture **toute** variante future, donc ajouter une valeur à
`Provenance` n'aurait pas cassé la compilation — ce que la décision affirmait
pourtant en toutes lettres. Le code écrit énumère donc les cinq variantes
interdites une par une. La garantie annoncée est désormais celle qui existe.

### La barrière qui manquait, et qui ne mordait nulle part

Le test réécrit **encadre** les `Managed` existants ; il n'en exige aucun. Sur
cette machine, où la branche de stratégie est absente, retirer l'appel à
`politique::marquer` laissait donc toute la suite verte — et il en va de même en
intégration continue, et sur toute machine non gérée. Le mécanisme aurait été
mort exactement comme avant, mais avec un test pour le certifier vivant.

Un second contrôle, **textuel** comme celui qui tient la liste des collecteurs,
exige que le collecteur appelle encore la ruche. Sa première rédaction cherchait
son motif par `contains` d'un littéral… présent dans sa propre assertion, que
`include_str!` relit. La falsification l'a laissée verte. Le motif est désormais
**assemblé à l'exécution**, et la falsification casse.

### Les injections, et ce qu'elles ont cassé

| Défaut injecté | Ce qui a cassé |
|---|---|
| un collecteur rendant `Keystone` | `un_releve_nest_ni_lauteur_de_la_valeur_ni_un_signal` |
| un collecteur rendant `Unknown` | le même, sur `is_security_signal` |
| `marquer` sans le contrôle de `CHEMINS` — donc `Managed` partout | le même, sur l'encadrement de `Managed` |
| l'appel à `politique::marquer` retiré du collecteur | `le_releve_de_la_branche_de_strategie_passe_par_la_ruche_de_politique` |

Chaque injection a été prouvée par `grep` **avant** d'exécuter le test, puis
restaurée et revérifiée. Une substitution qui rate ne remplace rien, et un test
vert sur du code non modifié ne prouve rien.
