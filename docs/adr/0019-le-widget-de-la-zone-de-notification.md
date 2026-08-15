# ADR-0019 — Ce qui vit dans la zone de notification, et ce qui n'y vit pas

- **Statut** : Accepté
- **Date** : 2026-08-16
- **Exigences concernées** : D16-06, P1, P6, P9, NF-01, brief §9 point 4

## Contexte

Le brief de design pose la question et refuse d'y répondre, en toutes lettres :

> **Widget de bureau / barre des tâches** — un anneau miniature dans la zone de
> notification, ou rien du tout ? La thèse du silence plaide pour « rien », mais
> un état à un coup d'œil a de la valeur.

Le point est resté ouvert des semaines. Il ne se tranche que maintenant, et pour
une raison précise : **jusqu'à cette semaine, il n'y avait rien à afficher.**
L'anneau du poste de pilotage répondait « pas encore calculable », faute d'état
de référence. Concevoir un indicateur permanent pour une valeur qui n'existe pas
aurait été concevoir sa décoration.

Depuis le lot 6 de la Phase 1, `ks import` écrit un `workstation.yaml` et
`ks diff` publie quatre verdicts. La posture devient une **mesure**. La question
a donc enfin un sujet.

## Ce qui rend la question difficile

Un widget de zone de notification est, par construction, **permanent et non
sollicité**. Il regarde l'utilisateur pendant qu'il travaille. C'est exactement
la surface où les produits de cette catégorie deviennent hostiles : le badge de
non-lus, la pastille rouge, la bulle qui s'ouvre pour annoncer qu'il n'y a rien.

Le dépôt a déjà tranché contre tout cela, à trois endroits :

- **D16-06** : au plus deux interruptions par jour, tout le reste agrégé dans le
  rapport matinal, et **aucun badge de non-lus**.
- **`tokens.css`** : le budget du rouge est de « ≤ 1 % de la surface de l'écran,
  et jamais en aplat ». Une icône de 16 px entièrement rouge dans une barre des
  tâches est un aplat.
- **La respiration de l'anneau ralentit** quand l'état se dégrade, au lieu de
  s'accélérer. Un instrument qui panique fait paniquer son pilote.

Un widget qui clignote, compte ou alerte contredirait les trois.

## Options

| Option | Ce qu'elle coûte, ce qu'elle rapporte |
|---|---|
| **Rien du tout** | Cohérent avec la thèse du silence, et gratuit. Mais elle a un coût réel : ouvrir une fenêtre pour apprendre qu'il n'y a rien à savoir est un geste que l'utilisateur finit par ne plus faire. Le produit devient invisible, donc inutile. |
| **Un badge avec un décompte** | La convention du secteur, et le seul choix explicitement interdit par D16-06. Un décompte de non-lus transforme un outil en dette d'attention. |
| **Une icône d'état, sans décompte ni notification** | Retenue. Trois tenues, aucune animation, aucune bulle, aucun chiffre. |
| **Un anneau miniature reproduisant le Health Ring** | Séduisant, et faux à 16 px : un arc partiel devient illisible, et un anneau plein se lit comme un chargement. La forme du cockpit ne se réduit pas. |

## Décision

**Une icône d'état dans la zone de notification. Trois tenues. Aucun décompte,
aucune notification non sollicitée, aucune animation.**

L'icône est le **logomark** — la clé de voûte de `design/marque/` — dans les trois
états que l'identité prévoit déjà :

| État | Ce que le dessin porte | Ce qu'il veut dire |
|---|---|---|
| Relevé, conforme | clé **pleine** | l'état lu correspond à l'état désiré |
| Écart actif | clé pleine, **contour épaissi** | au moins un écart demande une décision |
| Non calculable | clé **hachurée** | pas de référence chargée, ou lecture refusée |

Trois règles rendent ce choix tenable, et chacune ferme une dérive connue.

**1. Aucun chiffre.** L'icône dit qu'il y a quelque chose, jamais combien. Un
décompte invite à le regarder baisser, et c'est le mécanisme même du badge que
D16-06 interdit. Le nombre s'obtient en ouvrant, et il est alors accompagné de
sa raison.

**2. Aucune notification à l'initiative du widget.** Il ne consomme jamais l'une
des deux interruptions quotidiennes : elles appartiennent au rapport matinal et
aux constats graves, décidés ailleurs. Le widget est **consultable**, pas
parlant.

**3. Le survol dit l'essentiel en une phrase**, dans l'ordre du brief §1.3 —
ce qui s'est passé, ce que ça implique, ce qu'on peut faire. « 3 écarts depuis
hier 18 h. Aucun n'a d'auteur identifié. » Et jamais un code d'erreur nu.

**Le sens n'est jamais porté par la couleur seule** (NF-05) : les trois tenues
diffèrent par la **forme du dessin**, pas par sa teinte. C'est ce qui les rend
lisibles en contraste forcé, en vision déficiente, et à 16 px — les trois
conditions où une pastille colorée ne dit plus rien. La contrainte a d'ailleurs
déjà été payée : le logomark existe en monochrome et son dessin de petite taille
est distinct de celui de référence.

## Quand

**Phase 5**, avec l'interface, et pas avant. La raison n'est pas la difficulté :
c'est que l'icône affiche un verdict, et qu'un verdict suppose une référence
chargée et une série suivie. Livrer le widget avant que la posture soit une
mesure reviendrait à afficher en permanence « pas encore calculable », ce qui est
la définition d'un indicateur décoratif.

## Conséquences

### Ce que ça nous donne

Un état à un coup d'œil, sans dette d'attention. Et une réponse à la question du
brief qui ne tranche pas entre « rien » et « tout », mais qui nomme ce qui
distingue les deux : **l'icône informe quand on la regarde, elle ne réclame
jamais qu'on la regarde.**

### Ce que ça nous coûte

Une surface de plus à tenir, et une tentation permanente. Chaque demande future
d'y ajouter un chiffre, une bulle ou une couleur d'alerte devra être refusée en
citant cette ADR — c'est d'ailleurs sa principale raison d'exister.

Un coût technique aussi : une icône de zone de notification suppose un processus
résident, ce que le produit n'a pas aujourd'hui. C'est la même dépendance que la
détection événementielle de D2-04, et elle se décidera une fois pour les deux.

### Ce que ça ne garantit pas

**Que l'icône soit lisible.** Les trois tenues doivent être éprouvées à 16 px
sur les fonds réels d'une barre des tâches Windows, claire et sombre, et en
contraste forcé. Le logomark a déjà connu ce piège : une dent trop profonde le
faisait lire « M », et ça ne s'est vu qu'à l'écran. Rien ne dit que la
distinction entre « conforme » et « écart » survivra à la même épreuve — elle
repose sur une épaisseur de contour, ce qui est la différence la plus fragile à
petite taille.

**Que « rien du tout » soit une mauvaise réponse.** Elle reste défendable, et
cette ADR ne la disqualifie pas : elle choisit l'autre voie en assumant son coût.
Si l'épreuve des 16 px échoue, revenir à « rien » est une conclusion honnête, pas
un échec.
