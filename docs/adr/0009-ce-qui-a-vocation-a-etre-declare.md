# ADR-0009 — Ce qui a vocation à être déclaré, et ce qui n'est qu'un constat

- **Statut** : Proposé
- **Date** : 2026-08-02
- **Exigences concernées** : D2-02, D2-04, P6, moment de vérité M1

## Contexte

Le moment M1 est l'import de l'existant : Keystone écrit le premier
`workstation.yaml` à partir de la machine, sans qu'aucun formulaire soit rempli.
La tentation évidente est d'y verser les 115 items du scan. **Ce serait un
échec.** Un fichier de 115 lignes que personne ne relit ne dit pas ce qu'on veut ;
il redit ce qu'on a. La question n'est donc pas *comment écrire le yaml*, mais
*lesquels de ces items ont une raison d'y figurer*.

### Ce que produit réellement le scan

Relevé sur la machine de référence, le 2026-08-02, 115 items. Quatre familles
apparaissent, et elles ne se déclarent pas de la même façon :

| Famille | Exemples relevés | Combien |
|---|---|---|
| Faits sur la machine | `inventory.cpu.cores`, `security.firmware.version`, les 54 `inventory.software[*].version` | ~69 |
| Nombres qui bougent seuls | `inventory.uptime_seconds`, `space.volume[C:\].used_percent`, `security.defender.signatures_applied_at` | ~10 |
| États qu'on peut vouloir, qu'aucun verbe n'écrit | `security.platform.vbs_running`, `dma_protection_available`, `secure_boot` | 6 |
| Réglages | `security.services.windefend.startup`, `security.firewall.public.enabled`, `security.defender.realtime` | 28 |

**34 items déclarables sur 115.** Un fichier d'une trentaine de lignes, qu'un
humain relit en une minute. C'est le livrable de M1.

### Le second usage, qui décide de l'affaire

Le critère de sortie exige aussi la dérive suivie **sept jours sans faux positif
inexpliqué**. Sans cette distinction, le suivi produit du bruit dès le premier
jour :

- `inventory.uptime_seconds` change à **chaque scan** ;
- `space.volume[C:\].used_percent` change en permanence ;
- `security.defender.signatures_applied_at` change tous les jours — c'est même
  ce qu'on lui demande ;
- pire, `inventory.software[dbeaver2530currentuser].version` porte la version
  **dans son chemin** : une mise à jour de DBeaver ne produit pas un changement
  de valeur, elle produit une disparition et une apparition. Sur 54 applications,
  c'est une fabrique à faux positifs.

Un même axe répond aux deux questions. C'est ce qui le sauve du YAGNI : ce n'est
pas une abstraction posée pour plus tard, c'est une distinction dont deux
mécanismes de la Phase 1 dépendent immédiatement.

## Décision

`Item` porte une **nature**, décidée au point de construction, dans
`ks-collectors`. Quatre variantes, `match` exhaustif sans bras `_` chez tous les
consommateurs :

```rust
pub enum Nature {
    /// Un réglage : il a un état désirable, et un verbe du broker l'écrira.
    Reglage,
    /// Un état qu'on peut vouloir vrai, qu'aucun verbe n'écrit directement.
    /// Déclarable, suivi, mais **jamais convergeable**.
    Objectif,
    /// Un nombre qui évolue de lui-même. Ni déclaré ni suivi en Phase 1.
    Mesure,
    /// Un fait sur la machine. Ne se déclare jamais.
    Constat,
}
```

- `ks import` écrit les natures `Reglage` et `Objectif`, et rien d'autre.
- Le suivi de dérive ignore `Mesure` et `Constat`.
- La Phase 2 refusera de faire converger un `Objectif` : on ne prétend pas
  écrire ce qu'aucun verbe n'écrit.

Le critère vit **avec l'item**, dans le collecteur qui le fabrique, et nulle part
ailleurs. Les constructeurs `observed` et `item_posture` prennent la nature en
paramètre : on ne peut pas l'oublier, exactement comme on ne peut pas oublier
`purpose` et `risk`.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Une table de chemins déclarables dans l'importateur | deuxième source de vérité. Un collecteur qui ajoute un item le verrait **silencieusement** classé non déclarable, sans que rien ne casse — le mode de défaillance favori de ce projet |
| Déduire du préfixe du chemin (`security.` déclarable, `inventory.` non) | faux dès la première ligne : `security.firmware.version` est un constat, `virtualization.wsl[*].interop` est un réglage |
| Déduire de l'observation (une valeur qui bouge est une mesure) | demande un historique pour classer, donc classe mal les sept premiers jours — c'est-à-dire exactement la fenêtre du critère de sortie |
| Deux natures seulement, déclarable / non déclarable | perd `Objectif`, donc la Phase 2 tenterait de faire converger `vbs_running` ; et perd `Mesure`, donc `used_percent` finirait déclaré par égalité — un écart à chaque scan |
| Déclarer les 115 items | le fichier que personne ne relit. M1 se juge à la relecture, pas au remplissage |

## Conséquences

### Ce que ça nous donne

Un `workstation.yaml` d'environ trente lignes, relisible. Un suivi de dérive dont
le bruit est écarté **par une propriété de l'item**, pas par une liste
d'exceptions qui pourrit. Et une réponse à « pourquoi cet item n'est-il pas dans
mon fichier ? » qui tient en un mot, affichable par `ks explain`.

### Ce que ça nous coûte

Environ quarante sites de construction d'items à modifier dans `ks-collectors`,
et une décision à prendre **item par item** — sans valeur par défaut, parce
qu'une valeur par défaut est une décision qu'on n'a pas prise. Quelques cas
seront discutables : `security.platform.secure_boot` est classé `Objectif` et non
`Reglage`, faute de verbe capable d'écrire dans le firmware ; `hvci_uefi_lock`
est un `Reglage` alors que son verrou, une fois posé, ne se retire pas. Ces
arbitrages se documentent à côté de l'item.

### Ce que ça ferme

L'idée qu'un item déclaré soit forcément convergeable. `Objectif` acte qu'on peut
vouloir, mesurer et signaler sans savoir agir — ce qui est la position honnête
sur Secure Boot, la protection DMA et l'exécution effective de VBS.

### La dette prise, et sa condition de remboursement

Les items de nature `Mesure` ne sont **pas** déclarables en Phase 1, faute d'un
vocabulaire de contrainte (`≤ 85`, `plus récent que 7 jours`). Il n'y a
aujourd'hui qu'un seul cas d'usage réel, `space.volume[*].used_percent` : la
règle « pas de configuration avant le deuxième cas d'usage » s'applique. À écrire
le jour où un second apparaît, et pas avant.
