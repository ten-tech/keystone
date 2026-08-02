# Écrans et libellés — ce que la maquette d'exploration a produit

> Document de référence, extrait de l'import Claude Design avant sa suppression.
> L'import chargeait React, ReactDOM et Babel depuis un CDN à l'ouverture : il ne
> pouvait pas rester dans un dépôt dont le principe P5 est « Local, point final ».
> Sa **substance de conception** est ici, sous une forme diffable, relisible en
> revue, et autonome par construction.
>
> Il reste récupérable dans l'historique git (commit `91f2249`) si le rendu exact
> devait être reconsulté.

## Pourquoi ce document plutôt qu'une suppression sèche

Sur cinquante et un libellés de l'import, **trente-trois n'existaient pas** dans
`keystone-cockpit.html`. Supprimer sans extraire aurait perdu deux moments de
vérité du brief, la parité CLI de la palette de commandes, et une dizaine de
formulations qui appliquent correctement les cinq règles de voix.

## Les écrans, et ce qu'ils portent

### M1 — Premier lancement

Le moment de vérité qui décide de l'adoption. La phrase d'ouverture est la
thèse du produit en une ligne :

> **J'ai lu votre machine. Rien n'a été modifié.**

Elle dit l'ordre imposé par le brief — ce qui s'est passé, ce que ça implique,
ce qu'on peut faire — et elle le dit sans reproche, sans superlatif, sans point
d'exclamation. Aucun formulaire n'est présenté : l'état est **importé**, il ne
se saisit pas (D2-02).

Action proposée : **Adopter cet état comme référence**.

### M2 — Rapport matinal

> **état identique à 20:12. Rien à faire de votre côté.**

Le rapport qui ne dit rien est le rapport le plus fréquent, et c'est voulu :
le budget d'alertes est de deux interruptions par jour au maximum (D16-06). Un
outil qui parle tous les matins pour ne rien dire finit en règle de filtrage.

### Poste de pilotage

* **Posture composite**, avec **Déplier le calcul** et **Formule exacte** —
  application directe du principe P6 : un indicateur composite qu'on ne peut pas
  déplier en ses composantes exactes est une décoration.
* **Écarts avec l'état désiré**, colonnes *Catégorie*, *Origine*, *Voulu →
  constaté*, et **Ce que change l'écart**.
* **Prochaine fenêtre calme** — la maintenance se propose quand elle ne dérange
  pas, elle ne s'impose pas.
* **Dernières 24 heures**, colonnes *Heure*, *Événement*, *Source*, avec
  **Voir le récit complet**.
* Zone **Retour en arrière** distincte, avec **Revenu en arrière** et **Ce qui a
  échoué** — le rollback se raconte, il ne se subit pas (D3-10).

### Détail d'un item

* **Pourquoi cet item existe** — la finalité, obligatoire (P6, D1-10).
* **Ce que change l'écart** — la conséquence, pas la valeur brute.
* **Accepter la dérive** et **Accepter la dérive — note requise** : la note et
  l'expiration ne sont pas optionnelles (D2-06).
* **Faire converger cet item**, **Faire converger la sélection**.

### Plan de convergence

> **Plan de convergence — 5 correctifs, 3 vagues**

Avec **Tests de fumée qui jugeront le résultat** affichés *avant* l'application :
l'utilisateur sait à quoi le résultat sera jugé, donc ce qui déclenchera le
retour arrière automatique (D3-09, D3-10).

Et **Dernier plan appliqué**, pour que l'historique soit à un clic.

### Palette de commandes

Ouverte par **Ctrl K**. Deux traits à conserver absolument :

1. **Équivalent CLI** affiché à côté de chaque résultat. La CLI est la surface
   de référence (P4) ; l'interface ne doit jamais être le seul chemin vers une
   action, et le montrer apprend la CLI sans cours.
2. Le message de recherche vide :
   > **Rien ne correspond. La CLI accepte la même requête avec …**

   Un échec qui oriente au lieu de constater. C'est la meilleure ligne de tout
   l'import.

### Actions transverses

**Isoler la machine**, **Lever l'épingle**, **Surveiller**, **Voir le plan**,
**Voir le journal**, **Lire le fichier**, **Tout voir**.

## Ce que ce document ne remplace pas

Le rendu visuel. Les proportions, les rythmes d'animation et la densité se
jugent à l'œil, et `keystone-cockpit.html` reste la maquette de référence — la
nôtre, autonome, qui applique `tokens.css`.

Ce qui suit reste **à traiter** dans cette maquette, et n'était pas mieux résolu
dans l'import :

- [ ] `forced-colors` — attention, un style en ligne posé par un script l'emporte
      sur une règle `@media` sans `!important`
- [ ] cibles tactiles de la frise : 24 px minimum, contre 9 px aujourd'hui
- [ ] navigation au clavier des vagues et de la palette de commandes
- [ ] écran E7, absent des deux maquettes
- [ ] polices embarquées en `@font-face` local — décision à part : licence,
      graisses réellement utilisées, poids ajouté au dépôt
