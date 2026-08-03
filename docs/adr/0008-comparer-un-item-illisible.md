# ADR-0008 — Comparer un état désiré à une lecture qui a échoué

- **Statut** : Accepté — implémentée le 2026-08-02 (`Item::verdict`, `DriftSummary.unreadable`)
- **Date** : 2026-08-02
- **Exigences concernées** : D2-04, D2-05, P2, P6, SEC-01

## Contexte

La Phase 1 sort sur un critère précis : « la dérive suivie pendant sept jours
**sans faux positif inexpliqué** ». Le comparateur qui produira ces écarts existe
déjà, et il tient en cinq lignes — `Item::is_drifted`, `crates/ks-core/src/item.rs` :

```rust
match &self.desired {
    None => false,
    Some(want) => want != &self.observed,
}
```

Une égalité stricte sur `ItemValue`. Or `ItemValue` porte, depuis la Phase 0.2,
une variante `Illisible { raison }` qui n'est **ni une valeur ni une absence** :
elle dit qu'on n'a pas pu regarder. Sa documentation le dit sans ambiguïté, et
`ItemValue::est_constat` existe pour l'exclure des décomptes.

### Ce qui a été mesuré

Le comportement du comparateur, éprouvé le 2026-08-02 par un programme d'essai
reproduisant exactement les types du modèle :

| Comparaison | Résultat actuel |
|---|---|
| `desired = Bool(true)` contre `observed = Illisible{…}` | **écart** |
| `Illisible{« accès refusé »}` contre `Illisible{« WMI n'a pas répondu »}` | **différents** |
| `desired = Bool(true)` contre `observed = Absent` | écart |

Et l'état de la machine de référence, relevé le même jour par `ks scan` :

```
security.defender.exclusions.paths       illisible — accès refusé sans élévation
security.defender.exclusions.extensions  illisible — accès refusé sans élévation
security.defender.exclusions.processes   illisible — accès refusé sans élévation
```

Trois items, sur une clé protégée par ACL, dans une CLI qui ne s'élève jamais
(SEC-01). Ce n'est pas un cas limite : c'est le **cas nominal du produit**.

Conséquence directe : dès qu'on déclare une politique d'exclusions dans
`workstation.yaml` — et on la déclarera, c'est l'exigence D11-02 — Keystone
publie trois écarts par jour, tous les jours, indéfiniment. Aucun d'eux n'est
explicable : ils ne disent pas ce qui est faux, ils disent qu'on n'a pas
regardé. Le critère de sortie de la Phase 1 devient **inatteignable par
construction**.

### Le pendant, tout aussi grave

`DriftSummary::build`, `crates/ks-core/src/drift.rs` :

```rust
s.compliant = items_observed.saturating_sub(s.active + s.accepted + s.conflicts);
```

Un item illisible **non déclaré** est donc compté conforme. Un attaquant qui pose
une exclusion sous une clé que Keystone ne sait pas lire obtient un item vert.
C'est le défaut `wscvc` de la Phase 0.2, remonté d'un cran : `est_constat` a été
écrite précisément pour l'interdire, et elle n'a aujourd'hui **aucun appelant en
dehors du rapport HTML et des tests**.

## Décision

`Item::is_drifted() -> bool` est remplacée par `Item::verdict() -> Verdict`, une
énumération à quatre valeurs sur laquelle tout appelant fait un `match`
exhaustif, sans bras `_` :

```rust
pub enum Verdict {
    /// Aucun désir déclaré : Keystone ne contraint que ce qui est écrit.
    NonContraint,
    /// Le constat correspond au désir.
    Conforme,
    /// Deux constats qui diffèrent. Le seul cas convergeable.
    Ecart,
    /// Au moins un des deux côtés n'est pas un constat. Ni conforme, ni en écart.
    Incomparable { raison: String },
}
```

Règle unique : **si `!observed.est_constat()` ou `!desired.est_constat()`, le
verdict est `Incomparable`, quelle que soit l'autre valeur.** La raison est
reportée telle quelle, en clair, jusqu'à l'écran (P6).

`Absent` reste un constat — c'est déjà ce que dit `est_constat` — donc
`Absent` face à un désir non absent reste un `Ecart` légitime : la valeur devait
être là, elle n'y est pas.

`DriftSummary` gagne un champ `unreadable`, et `compliant` cesse de l'absorber :

```rust
s.compliant = items_observed - (s.active + s.accepted + s.conflicts + s.unreadable);
```

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Garder l'égalité stricte (état actuel) | trois faux positifs permanents sur la machine de référence, et le critère de sortie de la Phase 1 devient inatteignable |
| Compter l'illisible comme conforme | c'est le défaut `wscvc` : la clé qu'on ne sait pas lire devient la meilleure cachette du poste |
| N'autoriser la déclaration que des items lisibles | l'illisibilité dépend de la machine **et du porteur d'exécution**, pas de l'item : la même clé est lisible en intégration continue, où le porteur est administrateur, et refusée sur le poste. Un critère de déclarabilité qui dépend de l'exécution n'en est pas un — et la CI ne reproduirait jamais le défaut |
| Élever la CLI pour lire ces clés | contredit SEC-01, et déplace tout le produit dans le régime privilégié pour trois items |
| Renvoyer `Option<bool>` plutôt qu'un type nommé | `None` redirait « je ne sais pas » sans porter la raison, et le `match` exhaustif — la seule barrière qui casse la *compilation* — disparaîtrait |

## Conséquences

### Ce que ça nous donne

Un critère de sortie de Phase 1 atteignable. Un décompte de conformité qui ne
ment plus. Et, pour la Phase 2, la règle qui va de soi une fois écrite : **un
item incomparable n'est jamais convergeable** — on ne fait pas converger ce
qu'on n'a pas su lire, sous peine d'écraser une valeur qu'on n'a jamais vue.

### Ce que ça nous coûte

Une catégorie de plus à l'écran, qu'il faut savoir nommer sans reproche
(« illisible — accès refusé sans élévation », jamais « erreur »). La disparition
de `is_drifted` de l'API publique de `ks-core` : trois appelants, tous dans les
tests, plus une assertion dans `ks-collectors`. Et l'obligation de trancher, dans
`ks diff`, ce que voit l'utilisateur pour un incomparable : une ligne nommée,
jamais un agrégat.

### Ce que ça ferme

L'idée qu'un écart soit un booléen. Toute qualification future — seuil,
tolérance, comparaison approchée — passera par `Verdict` et devra s'y déclarer,
donc casser la compilation de ses appelants. C'est voulu.
