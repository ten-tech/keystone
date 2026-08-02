# ADR-0012 — Sortir l'interface du navigateur, et ce qu'elle a le droit d'afficher

- **Statut** : Accepté
- **Date** : 2026-08-02
- **Exigences concernées** : P4, P5, P6, SEC-01, NF-01, D16-01

## Contexte

Le cahier des charges §5 a tranché la technologie dès le premier jour :
**Tauri / WebView2**, pour « un seul binaire, empreinte mémoire faible, la même
base de code que la future vue mobile en lecture seule ». La feuille de route
place l'interface complète en **Phase 5**, sur huit semaines.

Deux faits ont rouvert le sujet plus tôt que prévu.

Le premier est un besoin d'usage : consulter l'état du poste en ouvrant un
fichier HTML depuis un navigateur est un détour. Le produit doit s'ouvrir comme
un logiciel s'ouvre.

Le second est un **risque de confusion**, et c'est lui qui structure cette
décision. `design/keystone-cockpit.html` est une maquette : la machine s'y
appelle `WKS-ORION-04`, la posture y vaut 94, l'inventaire y compte 312 items.
Rien de tout cela n'existe. Tant que c'est un fichier rangé dans `design/`,
personne ne s'y trompe. Empaqueté en exécutable, avec une icône dans la barre
des tâches, le même fichier devient un **produit qui affiche des chiffres
faux** — c'est-à-dire, exactement, le défaut que ce dépôt passe son temps à
corriger.

## Options

| Option | Ce qu'elle coûte, ce qu'elle rapporte |
|---|---|
| **Attendre la Phase 5** | Cohérent avec la feuille de route, et gratuit. Mais la coque est indépendante des sept écrans : rien n'oblige à les livrer ensemble, et on se prive cinq phases durant d'un usage réel du seul artefact déjà branché sur de vraies données. |
| **Fenêtre applicative d'un navigateur** (`--app=`) | Zéro dépendance, disponible tout de suite. Mais ce reste un navigateur déguisé : pas d'icône propre, pas de cycle de vie, rien de ce qui distingue un logiciel. Et Edge n'est pas installé sur le poste de référence, ce qui rend le procédé non reproductible. |
| **Coque Tauri affichant la maquette** | La plus rapide à faire impression, et la seule à écarter absolument : elle transforme une fiction en produit. Un utilisateur qui lit « posture 94 » dans une fenêtre intitulée Keystone croit lire sa machine. |
| **Coque Tauri affichant le rapport réel** | Retenue. Le rapport est déjà produit par le code, à partir de 115 items réellement lus sur la machine. La coque ne fabrique aucune donnée : elle déplace un artefact vrai hors du navigateur. |

## Décision

**`ks-ui`, coque Tauri 2, affiche le HTML produit par `rapport::construire` à
partir d'un `Inventory::collect_all()` réellement exécuté. Elle n'affiche jamais
la maquette.**

Quatre conséquences de forme découlent de ce choix, et chacune a une raison.

**1. `ks-ui` vit dans un workspace séparé, sous `ui/`, exclu de la racine.**
Tauri tire environ 415 crates ; le workspace principal en compte 123. Les verser
dans le verrou partagé ferait payer à chaque job de CI, et à chaque
`cargo test --workspace`, une surface qui ne les concerne pas. Le critère
d'admission est mesurable : après l'ajout, `Cargo.lock` de la racine ne gagne
**aucune** entrée.

**2. `rapport` devient un module de bibliothèque de `ks-cli`, pas une copie.**
Deux générateurs de rapport divergeraient, et le jour où ils divergeraient,
c'est la version affichée à l'écran qui aurait raison contre celle qu'on aurait
relue.

**3. La coque est non privilégiée et en lecture seule**, comme la CLI (SEC-01).
Elle ne s'installe jamais en service. Les capacités que Tauri offre par
commodité — accès au système de fichiers depuis le frontend, plugin shell —
sont désactivées : chacune serait une surface ajoutée sur un produit dont le
modèle de menace est déjà écrit.

**4. Aucune ressource réseau**, principe P5. La contrainte est déjà tenue par le
rapport, qui est autonome par construction, et déjà vérifiée par une étape de
CI.

## Le piège de MSRV, quatrième occurrence

Tauri 2.11.5 déclare `rust-version = "1.77.2"`. Son arbre transitif, lui, exige
**1.88** : `darling_macro` 0.23 réclame 1.88, `icu_collections` et
`icu_locale_core` 2.2 réclament 1.86. Mesuré, pas déduit —
`cargo +1.85.0 check --locked` échoue avec « rustc 1.85.0 is not supported by
the following packages ».

C'est la **quatrième fois** que ce piège se referme sur ce projet, après
`libsqlite3-sys` 0.38, `wmi` 0.18 et `sysinfo` 0.39.

Il ne s'est pas refermé cette fois. Le passage du workspace au **résolveur 3**,
décidé le matin même, fait choisir à cargo les versions compatibles avec la
MSRV déclarée : `icu_collections` 2.1.1 au lieu de 2.2.0, et ainsi de suite.
Vérifié en compilant réellement — `cargo +1.85.0 check --locked` sur un arbre
verrouillé par le résolveur 3 : **415 paquets, 3 min 14 s, propre**.

Sans cette décision, la réponse à ce besoin aurait été « relever la MSRV à
1.88 », c'est-à-dire fermer la porte à des postes plus anciens pour une raison
d'interface. Le résolveur ne supprime pas le risque — douze dépendances de
l'arbre principal ne déclarent aucune `rust-version`, et le résolveur ne peut
rien comparer sur celles-là — mais il vient d'éviter une décision lourde prise
pour une mauvaise raison.

Le runtime WebView2 est présent sur le poste de référence, en 150.0.4078.105.

## Conséquences

### Ce que ça nous donne

Le premier artefact du produit qui s'ouvre comme un logiciel, sur des données
vraies. Et un socle réutilisable pour la Phase 5 : les sept écrans du brief
viendront dans cette coque, pas à côté.

### Ce que ça nous coûte

Une surface de dépendances multipliée par trois et demi, isolée mais réelle, à
suivre au même titre que le reste. Un second workspace, donc un second verrou,
un second `cargo audit`, un second `cargo deny`. Et un chemin de code de plus
entre la collecte et l'écran, qu'il faudra garder aligné sur la CLI — c'est la
raison de la décision n° 2.

### Ce que ça ne garantit pas

**La coque n'est pas la Phase 5.** Elle affiche un rapport, pas les sept écrans
du brief. Le Health Ring, la vue Dérive, la palette de commandes et la parité
CLI affichée restent à écrire, et la maquette reste une maquette. Écrire
l'inverse ferait de cette ADR le mensonge qu'elle prétend éviter.

### Ce que ça ferme

Rien. La maquette continue d'exister comme maquette, dans `design/`, et c'est là
que se conçoivent les écrans avant d'être branchés. Le jour où un écran affiche
des données réelles, il entre dans la coque et sort de la maquette — la
frontière est cette question-là, et pas une autre.
