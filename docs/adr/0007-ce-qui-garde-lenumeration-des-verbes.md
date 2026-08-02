# ADR-0007 — Ce qui garde l'énumération des verbes, et jusqu'où

- **Statut** : Accepté
- **Date** : 2026-08-02
- **Exigences concernées** : SEC-02, SEC-03, P6
- **Complète** : [ADR-0006](0006-fermer-les-verbes-a-parametres-libres.md)

## Contexte

L'ADR-0006 a fermé les deux verbes à paramètres libres. La revue qui a suivi a
duré **sept passes** et produit **dix-neuf tentatives de contournement**, dont
douze construites par le gardien du broker et sept par moi. Toutes ont été
éprouvées par injection réelle dans une copie jetable, puis restaurées.

Le résultat compte moins que sa forme. Les contournements se rangent en quatre
familles, et **chaque correctif fermait un membre en laissant la famille
intacte** :

| Famille | Ce qui était attaqué | Exemples |
|---|---|---|
| Délimitation | *comment* on lit le source | accolade fermante dans un commentaire de ligne, puis dans un commentaire de bloc, puis dans un littéral de chaîne d'un attribut `#[doc = "…"]` |
| Ancre | *quel élément* on lit | leurre `#[cfg(any())] mod … { pub enum Verb }` déclaré plus haut ; puis leurre rendu **unique** par `pub use crate::Commande as Verb;` |
| Extraction | *quels champs* on voit | variante *newtype* `SetTuning(Tuning)`, sans couple `nom: type` |
| Projection | *ce que le type expose* | `#[serde(skip_serializing)]`, `#[cfg_attr(all(), serde(rename))]`, `#[serde(flatten)]` |

Le fait marquant est la troisième famille. Après trois passes à durcir la
manière de lire, l'attaque a cessé de viser la lecture pour viser son sujet :
une barrière qui désigne ce qu'elle garde **par un nom écrit dans le texte**
peut toujours se faire présenter un autre texte.

Et la quatrième a révélé un défaut qui n'a rien à voir avec l'évasion. Un
paramètre marqué `skip_serializing` est un paramètre **que le journal ne peut
pas enregistrer** : SEC-03 exige « appelant, verbe, paramètres, diff,
résultat », et toute implémentation fondée sur `serde` l'omettrait en silence.
Un contributeur qui ajoute `skip_serializing_if` pour ne pas journaliser une
chaîne vide retirerait ce paramètre de la piste d'audit **sans aucune
malveillance et sans qu'aucun leurre soit nécessaire**.

## Décision

**Trois choses, et il ne faut pas les confondre.**

1. La barrière textuelle reste, dans son état actuel : huit contrôles, ancrés
   au niveau du **champ** contre le type compilé, et verrouillant le **droit de
   configurer les projections**. Elle est considérée comme arrivée à maturité.
2. Les quatre `String` restants de `Verb` — `kind`, `target`, `snapshot_id`,
   `expires` — **seront typés**, avant la première écriture de la Phase 2. C'est
   la dette déjà nommée par l'ADR-0006, et c'est **la décision de sécurité**.
3. L'adoption de `syn` comme dépendance de développement est **la décision
   d'outillage**. Elle est différée, et sa justification écrite ci-dessous pour
   qu'on n'ait pas à la reconstruire.

Ces trois éléments ne se substituent pas les uns aux autres. Confondre le
deuxième et le troisième dans un même paragraphe est l'erreur que cette ADR
existe pour empêcher.

## Ce que la barrière textuelle garde réellement

Elle est la **troisième ligne**, et il faut le dire clairement pour ne pas lui
prêter un rôle qu'elle n'a pas.

* La première est le `match` exhaustif de `nom_du_verbe`, sans bras `_` : une
  garantie du **compilateur**, qui casse la CI avant qu'un test s'exécute.
* La deuxième est l'égalité avec la liste que le derive `Deserialize` produit à
  partir du type, que le texte ne contrôle pas.
* La troisième est la lecture du source. Son rôle est de **rendre l'évasion
  bruyante**, pas de la rendre impossible.

Dix-neuf tentatives ont montré qu'écrite à la main elle ne le fait
qu'imparfaitement. C'est un constat empirique, pas une préférence.

## Alternatives écartées, ou différées

| Voie | Décision |
|---|---|
| **Ne rien faire de plus** | Refusé. La dette des quatre `String` est réelle et datée ; `TakeSnapshot { kind, target }` fait aujourd'hui écrire par le broker, en SYSTEM, la ruche des comptes locaux. |
| **Adopter `syn` maintenant** | Différé. Le coût en dépendances est **nul** — `syn`, `proc-macro2`, `quote` et `unicode-ident` sont déjà dans `Cargo.lock`, tirés par `serde_derive`, `clap_derive`, `thiserror-impl` et `windows-implement`, vérifié dans le verrou. Mais `syn` améliore l'auto-inspection du produit, pas le produit. À faire après le typage, pas avant. |
| **Typer les quatre champs** | **Retenu**, avec l'échéance de l'ADR-0006. |
| **Remplacer la barrière textuelle par la revue seule** | Refusé. La revue humaine fatigue ; un test qui échoue, non. |

## Ce que `syn` fermerait, et ce qu'il ne fermerait pas

À écrire maintenant, tant que les dix-neuf tentatives sont fraîches.

**Il fermerait les familles 1, 2 et 3 par construction.** Il n'y a plus de texte
à délimiter : `parse_file` rend un arbre. Le leurre devient une donnée qu'on
inspecte — on exige un unique `enum` dont le nom public est `Verb` après
résolution des alias visibles — au lieu d'un piège dans lequel on tombe. Les
variantes *tuple* et *newtype*, les alias de type, les génériques et les
attributs, y compris sous `cfg_attr`, deviennent des structures qu'on interroge.

**Il ne fermerait rien de sémantique.** Il dirait que
`TakeSnapshot { kind: String }` existe ; il ne dirait jamais que
`registry-export` sur `HKLM\SAM` est une extraction d'empreintes de comptes.
`RunScript { path }` sous un nom anodin resterait l'angle mort que le code
documente déjà.

**Il ne réglerait pas non plus le défaut SEC-03**, qui est une question de
journalisation, pas de paramètre libre. Il rendrait seulement son contrôle
trivial : le parcours de chaînes deviendrait une inspection d'attributs.

**Et il apporterait son propre mode de panne** : une erreur d'analyse devient un
test rouge dont le message parle de grammaire, pas de sécurité.

## Conséquences

### Ce que ça nous donne

Une doctrine écrite plutôt qu'un réflexe. Le prochain contributeur qui trouvera
la barrière textuelle laborieuse saura pourquoi elle existe, ce qu'elle ne
promet pas, et quelle est la vraie sortie.

### Ce que ça nous coûte

Le typage des quatre champs est du travail réel : `SnapshotId` validé par
construction et jamais concaténé à un chemin, `Expiry` qui refuse une date
passée et borne l'horizon, `ExclusionPath` qui refuse les racines de volume et
les jokers et **résout les variables d'environnement dans le contexte
LocalSystem avant de produire le diff**. Ce dernier point n'est pas cosmétique :
le service Defender tourne sous LocalSystem, donc `%TEMP%` s'y résout en
`C:\Windows\TEMP`. Un paramètre transmis verbatim produirait un diff qui ne
désigne pas le dossier réellement exclu, c'est-à-dire une simulation qui ment.

### Ce que ça ferme

Rien. `syn` reste ouvert, et la présente ADR sera complétée le jour où il sera
adopté, pas remplacée.

### Ce qui reste, et qui est le vrai risque

Trois verbes gardent des paramètres libres : `TakeSnapshot`, `RestoreSnapshot`,
`AddDefenderExclusion`. **La barrière ne les couvre pas.** Ils sont nommés et
datés dans l'ADR-0006, avec leur danger décrit et leur correctif visé. C'est là
qu'est le risque résiduel du broker aujourd'hui — pas dans une vingtième ruse
d'analyse lexicale.

## La phrase à retenir

**La barrière textuelle a atteint ce qu'elle peut atteindre. Ce qui protégera le
broker ensuite, ce sont les quatre types qui restent à écrire.**
