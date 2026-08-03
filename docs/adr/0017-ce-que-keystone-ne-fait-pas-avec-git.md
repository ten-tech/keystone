# ADR-0017 — Ce que Keystone ne fait pas avec git en Phase 1

- **Statut** : Proposé
- **Date** : 2026-08-03
- **Exigences concernées** : D2-01, D2-06, D2-07, P2, P3, SEC-02 (par analogie)

## Contexte

La feuille de route porte, en Phase 1 :

> - [ ] Intégration git : commit automatique du yaml à chaque changement accepté

La phrase est courte et elle cache trois gestes très différents : écrire un
fichier, décider quoi commettre, et **lancer un processus**. Les trois méritent
d'être regardés séparément.

### Où vit le fichier, mesuré

`.gitignore`, à la racine de ce dépôt :

```
# ─── Configuration réelle du poste ────────────────────────────
/workstation.yaml
/base.yaml
```

Et `docs/08-CONVENTIONS.md` § « Les deux dépôts » :

| Dépôt | Contenu |
|---|---|
| celui-ci | le code, la documentation, le schéma, un **exemple fictif** |
| le dépôt du poste | le `workstation.yaml` **réel**, privé, séparé, chez l'utilisateur |

Le dépôt cible du commit n'est donc **pas celui-ci**. C'est un dépôt qui, sur
cette machine, **n'existe pas encore**. Écrire aujourd'hui « commit automatique »
suppose de savoir où, ce que personne ne sait.

### `git commit` exécute du code arbitraire

C'est le fait qui décide de cette ADR, et il n'est pas une subtilité.

`git commit` déclenche les crochets du dépôt : `pre-commit`, `prepare-commit-msg`,
`commit-msg`, `post-commit`. Ce sont des fichiers exécutables posés dans
`.git/hooks/`, et git les lance sans rien demander. Un `git commit` déclenché par
Keystone est donc **Keystone exécutant un fichier arbitraire du disque**.

`ks-cli` n'est pas `ks-broker`, et SEC-02 ne s'y applique pas à la lettre. Mais
la doctrine, elle, s'y applique : le projet a écrit noir sur blanc que
`RunScript { path }` est « une porte dérobée, avec un détour ». Un crochet git
est exactement cela, avec un détour de plus. La question éliminatoire —
*ce geste permet-il, directement ou par détour, d'exécuter du code arbitraire ?*
— reçoit ici un **oui** franc.

### Les deux façons de commettre, et ce qu'elles coûtent

**Lancer `git.exe`.** Aucun précédent dans le code de production : `Command::new`
n'apparaît que dans `ks-cli/tests/integration.rs`, pour lancer la CLI elle-même.
Le PATH décide quel `git` s'exécute ; sur un poste de développement, ce n'est pas
une hypothèse théorique.

**Lier une bibliothèque.** Mesuré le 2026-08-03 : `git2` 0.21.0 (liaisons vers
libgit2, du C, donc `unsafe` dans l'arbre) ou `gix` 0.86.0 (pur Rust, mais un
arbre de dépendances qui dépasse le workspace entier). Pour commettre un fichier
de trente lignes.

### Ce qu'un commit automatique casse, du côté du principe P3

Un commit est réversible, on peut le concéder : `git reset --soft HEAD~1`.
Mais **seulement si rien d'autre n'a eu lieu entre-temps**, et Keystone ne
contrôle pas ce dépôt. Il ne contrôle ni l'index, ni les fichiers en cours de
modification, ni la branche courante, ni la présence d'un rebase interrompu. Un
`git commit -a` déclenché par Keystone emporterait le travail non terminé de
l'utilisateur dans un commit qu'il n'a pas demandé — et le retour arrière de
*cela*, Keystone ne sait pas le fabriquer.

Or la réversibilité est un **critère d'admission** d'une fonctionnalité, pas une
qualité souhaitable (P3).

## Décision

**En Phase 1, Keystone n'exécute jamais git.**

`ks import` et `ks accept` écrivent le fichier, puis **affichent la commande** à
exécuter, avec le message de commit rédigé selon les conventions du projet :

```
Fichier écrit : D:\poste\workstation.yaml
  34 déclarations, 3 items non déclarés car illisibles.

Pour le versionner :
  git -C D:\poste add -- workstation.yaml
  git -C D:\poste commit -m "chore(config): adopte l'état lu du 2026-08-03 (D2-02)"
```

Trois précisions qui ne sont pas décoratives :

**1. L'écriture du fichier, elle, a lieu — et elle ne détruit rien.** `ks import`
refuse d'écraser un fichier existant, comme `ks report` le fait déjà, et pour la
même raison (P3). `--force` existe et doit être passé.

**2. L'écriture est atomique.** Fichier temporaire dans le même répertoire, puis
renommage. Un `ks import` interrompu ne laisse pas un `workstation.yaml`
tronqué, qui serait la source de vérité d'un outil de sécurité amputée en
silence.

**3. La décision, elle, est journalisée.** D2-07 exige une trace durable de
chaque acceptation de dérive. Cette trace est une entrée du **journal de
Keystone**, chaînée, dans son propre magasin — pas un commit git. Le journal
existe, il est éprouvé, il est ancré sur la genèse. Faire dépendre une exigence
de traçabilité d'un dépôt externe qui n'existe pas encore serait la remplacer par
une intention.

**4. La ligne de la feuille de route est réécrite dans le même commit** que la
présente ADR :

> - [x] Le yaml est écrit atomiquement, sans écrasement implicite ; la commande
>   git est **proposée**, jamais exécutée (ADR-0017)

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| `git commit` automatique via `Command::new("git")` | déclenche les crochets du dépôt, c'est-à-dire l'exécution de code arbitraire du disque, déclenchée par Keystone. C'est le geste que la doctrine du projet refuse sous le nom `RunScript { path }` |
| Idem, avec `--no-verify` pour désactiver les crochets | désactive `pre-commit` et `commit-msg`, pas `post-commit` ni les filtres `clean`/`smudge` de `.gitattributes`. Une atténuation partielle présentée comme une garantie est pire que pas d'atténuation |
| `git2` (libgit2) pour commettre sans lancer de processus | ferme la question des crochets, ouvre celle du C : `unsafe` dans l'arbre de `ks-cli`, pour un service que le produit peut rendre en affichant deux lignes |
| `gix`, pur Rust | même service, et un arbre de dépendances qui dépasse celui du workspace entier. Le rapport coût/bénéfice n'est pas défendable pour commettre un fichier |
| Un drapeau `--commit`, explicite, non automatique | c'est le compromis raisonnable, et il reste atteignable. Mais il ne se justifie qu'une fois le dépôt du poste créé et son chemin connu — donc pas avant le deuxième cas d'usage. On ne configure pas avant d'avoir le besoin |
| Poser un crochet `post-commit` côté utilisateur pour appeler Keystone | inverse la dépendance, ce qui est plus sain, mais fait de Keystone une cible d'exécution déclenchée par git. Et c'est une écriture dans `.git/hooks`, donc une écriture système en Phase 1 |
| Faire du commit git la trace de D2-07 | subordonne une exigence de traçabilité à un dépôt externe optionnel, réinscriptible par l'utilisateur, et sans chaînage. Le journal de Keystone fait déjà mieux |

## Conséquences

### Ce que ça nous donne

La Phase 1 conserve la propriété qui fait qu'on peut la lancer sur une machine
de production : le seul fichier que Keystone écrit est le sien, plus celui qu'on
lui demande explicitement d'écrire. Aucun processus n'est lancé, aucune
bibliothèque C n'entre dans l'arbre, aucun crochet inconnu ne s'exécute. Et
D2-07 est tenue par un mécanisme qui existe et qui est éprouvé.

### Ce que ça nous coûte

Deux lignes à copier-coller après chaque acceptation de dérive. Sur sept jours
d'observation, cela représente quelques occurrences. C'est un coût réel et
mesurable, et il est jugé inférieur à celui d'exécuter du code inconnu.

Et l'aveu que le moment M1 s'arrête au fichier : l'utilisateur voit son état
adopté, il ne voit pas son premier commit. La démonstration est moins jolie.

### Ce que ça ferme

Rien. `--commit` reste écrivable en Phase 2, quand le moteur d'instantanés
existera — un commit y devient une variante d'instantané, avec un retour arrière
que Keystone sait produire et un journal qui l'enregistre. C'est le bon moment,
et ce n'est pas maintenant.

### Ce que ça ne garantit pas

**Rien n'empêche l'utilisateur de ne jamais commettre.** Le fichier vit alors
hors de tout historique, et D2-01 — « source de vérité unique, versionnée en
git » — n'est tenue que par la discipline de son propriétaire. Keystone peut
constater l'absence de dépôt et le dire ; il ne peut pas y remédier sans faire
exactement ce que cette ADR refuse.

**L'écriture atomique protège de l'interruption, pas de la concurrence.** Deux
`ks import` simultanés se terminent par deux renommages, et le dernier gagne.
C'est acceptable pour une commande interactive ; ce ne le serait plus si un
ordonnanceur l'appelait.

**Le message de commit proposé est du texte affiché.** Rien ne vérifie qu'il
sera employé, ni que la référence d'exigence qu'il porte correspond au contenu.
La convention de commit du projet reste tenue par la revue, ici comme ailleurs.
