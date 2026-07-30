# KEYSTONE — Brief de design
### Direction « Poste de pilotage » · document destiné à Claude Design

**Version** 1.0 · **Date** 30 juillet 2026 · **Auteur** Ingénierie poste de travail
**Livrable attendu** un design system complet + 7 écrans maquettés, mode sombre natif
**Document jumeau** `KEYSTONE-CDC.md` (cahier des charges fonctionnel et technique)

---

## 0. À lire avant toute chose

Keystone est un **plan de contrôle pour poste de travail d'ingénieur** : il surveille, décrit et fait converger l'état d'une machine Windows 11 avec ses distributions WSL2 et ses VM Hyper-V. Il met à jour, nettoie, durcit, sauvegarde, détecte les changements non désirés, et sait tout annuler.

Ce n'est pas un utilitaire. C'est **l'instrument par lequel un ingénieur regarde sa machine**, plusieurs fois par jour, pendant des années. Le design doit donc tenir la répétition : ce qui est spectaculaire au jour 1 doit encore être supportable au jour 900.

> **Thèse émotionnelle du produit — une seule phrase :**
> **Le calme est la fonctionnalité.**
> La récompense que Keystone offre à son utilisateur n'est pas une animation, un badge ou un score : c'est un écran silencieux qui respire, et la certitude que si quelque chose bougeait, il le saurait.

Tout choix de design se juge à cette aune. Si un élément crée de l'anxiété pour attirer l'attention, il est faux. Si un élément récompense l'utilisateur par de la stimulation plutôt que par de la tranquillité, il est faux.

---

## 1. Où va l'émotion — les cinq leviers

L'émotion en interface ne vient pas des dégradés. Elle vient de cinq leviers, dans cet ordre d'importance.

### 1.1 Le rythme
Le tempo de l'interface est ce que le corps perçoit avant l'œil.

- **La respiration.** L'élément central (le *Health Ring*, §4.1) pulse à **0,25 Hz — un cycle de 4 secondes**, soit la fréquence respiratoire d'un adulte au repos. C'est une fréquence que le système nerveux reconnaît comme apaisée. Ce n'est pas décoratif : c'est un signal physiologique.
- **Le contre-instinct qui fait tout le produit :** quand l'état se dégrade, la respiration **ralentit** (cycle porté à 6,5 s) au lieu de s'accélérer ou de clignoter. Un instrument qui panique fait paniquer son pilote. Keystone devient plus lent, plus grave, plus posé quand ça va mal. C'est la signature de la marque.
- **Barème de durées, strict :**

| Rôle | Durée | Easing |
|---|---|---|
| Micro-feedback (toggle, bouton, cran) | 120 ms | `cubic-bezier(.2,.8,.2,1)` |
| Transition d'état, ouverture de panneau | 200 ms | `cubic-bezier(.2,.8,.2,1)` |
| Apparition de contenu, entrée en scène | 400 ms | `cubic-bezier(.16,1,.3,1)` |
| Respiration | 4 000 ms (6 500 ms dégradé) | sinusoïdal `ease-in-out` |
| Révélation du rapport matinal | 700 ms, cascade de 60 ms | `cubic-bezier(.16,1,.3,1)` |

- **Règle absolue :** une animation peut *couvrir* une latence, jamais l'*ajouter*. Si l'action est instantanée, l'interface est instantanée.

### 1.2 La matière
La profondeur vient de la **lumière**, pas de l'ombre portée.

- Trois plans de surface seulement (§3.2), séparés par des liserés `1px rgba(255,255,255,.07)` — jamais de bordure franche, jamais de `box-shadow` noir diffus.
- **Le verre du cockpit :** les panneaux flottants portent un `backdrop-filter: blur(20px) saturate(140%)` sur une surface à 78 % d'opacité. Utilisé pour la palette de commandes et les feuilles modales uniquement.
- **Le grain.** Un bruit monochrome à **2 % d'opacité** en overlay global (`mix-blend-mode: overlay`), en SVG `feTurbulence` inline. Deux raisons : il tue le *banding* des dégradés sombres sur écran 8 bits, et il donne à l'écran une texture de matériel plutôt que de logiciel. Non négociable, et invisible si bien fait.
- **La lumière vient du haut.** Un halo radial très diffus en haut de la vue principale (`radial-gradient` à 4 % d'opacité). L'écran a une source lumineuse cohérente.
- **Zéro skeuomorphisme.** Pas de fausse vis, pas de faux cuir, pas de reflet de verre dessiné. On évoque l'instrument de précision par la *retenue*, pas par l'imitation.

### 1.3 La voix
Le wording porte plus d'émotion que la couleur. Keystone parle comme un copilote expérimenté : factuel, calme, jamais moralisateur.

**Les cinq règles de la voix :**
1. On dit **ce qui s'est passé**, puis **ce que ça implique**, puis **ce qu'on peut faire**. Dans cet ordre, jamais l'inverse.
2. Jamais de code d'erreur nu. Un code technique est toujours replié derrière « Détails ».
3. Jamais de reproche. L'utilisateur n'a pas « oublié » de mettre à jour ; la mise à jour « est disponible depuis 12 jours ».
4. Jamais de superlatif d'urgence : pas de « CRITIQUE ! », « URGENT », « IMMÉDIATEMENT », pas de majuscules criées, pas de point d'exclamation. La gravité se dit par le mot juste, pas par la typographie.
5. Le chiffre avant l'adjectif. « 47 Go récupérables » vaut mieux que « beaucoup d'espace gaspillé ».

**Avant / après :**

| ✗ Interdit | ✓ Attendu |
|---|---|
| ⚠️ MENACE DÉTECTÉE ! Votre système est en danger | Une autorité de certification racine a été ajoutée hier à 23:14. Je ne la reconnais pas. Tant qu'elle est présente, votre trafic HTTPS peut être lu. |
| Erreur 0x80070005 | Je n'ai pas pu écrire la règle de pare-feu — le service a refusé l'accès. *Détails : 0x80070005 · ACCESS_DENIED* |
| Optimisation en cours… | Compaction du disque de la distro `ubuntu-dev` — 3,1 Go sur 8,4 Go traités |
| Vous n'avez pas redémarré depuis 23 jours | 3 correctifs attendent un redémarrage. La prochaine fenêtre calme est ce soir 20:10. |
| Tout va bien 🎉 | Rien n'a changé depuis hier. |

### 1.4 La récompense
Il faut **designer explicitement** les instants où le produit rend quelque chose à l'utilisateur (§5, les Moments de vérité). Une fonctionnalité utile mal mise en scène ne crée aucun attachement.

La récompense est toujours **un fait tangible**, jamais une félicitation : des gigaoctets qui reviennent, un rollback raconté, un anneau qui retrouve son rythme.

### 1.5 Le silence
C'est le levier que 99 % des outils de ce genre manquent.

- **Zéro badge de notifications non lues.** Jamais.
- **Zéro gamification** : pas de série de jours, pas de trophée, pas de « niveau de sécurité ».
- Un **budget d'alertes** : au maximum **2 interruptions par jour**, tout le reste s'agrège dans le rapport matinal. Une alerte qui ne mérite pas d'interrompre n'existe pas ; elle attend.
- Quand tout va bien, la vue d'ensemble contient **très peu de choses** — et c'est le but. Le vide est un livrable.

---

## 2. Anti-patterns émotionnels — interdits contractuels

Ces points ne sont pas des préférences. Un livrable qui en contient un est à refaire.

| Interdit | Pourquoi |
|---|---|
| **Le dark pattern de la peur** (« 3 MENACES ! » en rouge, jauge de risque anxiogène) | C'est le modèle économique des antivirus grand public. Il détruit la confiance de l'utilisateur expert et rend l'outil illisible : quand tout crie, plus rien ne s'entend. |
| **Rouge comme couleur d'accent ou de remplissage de zone** | Le rouge doit rester si rare qu'il fasse lever la tête. Budget : ≤ 1 % de la surface, et seulement pour un état confirmé grave. |
| **Clignotement, pulsation rapide, secousse** | Risque vestibulaire, et contraire à la thèse. Rien ne clignote jamais dans Keystone. |
| **Score de santé sur 100 sans définition** | Un chiffre non explicable est une décoration. Tout indicateur composite doit pouvoir se déplier en ses composantes exactes. |
| **Badge / compteur de non-lus** | Fabrique une dette d'attention. |
| **Modal pendant une réunion, une présentation ou un build** | Le produit connaît le contexte : il attend. |
| **Spinner sans information** | Toujours dire *quoi*, et si possible *combien*. |
| **Animation sur le chemin critique** | On n'attend jamais l'interface. |
| **Émoji dans l'UI produit** | Les états passent par icône vectorielle + libellé texte. |
| **Icône seule pour un état** | Toujours icône **+** libellé. L'état n'est jamais porté par la couleur ou la forme seule (§6). |

---

## 3. Fondations

### 3.1 Typographie

| Rôle | Police | Notes |
|---|---|---|
| Titres, chiffre héros | **Inter Tight** | `letter-spacing: -0.02em` sur les gros corps. Chiffres **proportionnels** sur le chiffre héros et les tuiles — jamais `tabular-nums` en grande taille, ça fait flotter les nombres. |
| Corps, libellés, UI | **Inter** | |
| Télémétrie, tables, chemins, code, ticks d'axe | **JetBrains Mono** | `font-variant-numeric: tabular-nums` **ici uniquement** : c'est là que les chiffres doivent s'aligner verticalement et ne pas faire sauter la mise en page quand ils changent en direct. |

Aucune police à empattement, aucune police display. L'instrument de précision se signale par la neutralité.

**Échelle** (base 16, ratio 1.25 tempéré) :

| Token | Taille / interligne | Usage |
|---|---|---|
| `--fs-hero` | 76 / 1.0 | chiffre héros au centre du Ring |
| `--fs-display` | 34 / 1.15 | chiffre de tuile vitale |
| `--fs-h1` | 22 / 1.3 | titre d'écran |
| `--fs-h2` | 16 / 1.4 | titre de panneau |
| `--fs-body` | 14 / 1.55 | corps |
| `--fs-sm` | 13 / 1.5 | secondaire, aide |
| `--fs-mono` | 12.5 / 1.6 | télémétrie, chemins |
| `--fs-label` | 11 / 1.2 | sur-titres, `letter-spacing: .09em`, capitales |

### 3.2 Couleur — palette validée

**Toutes les valeurs ci-dessous ont été vérifiées par calcul** (contraste WCAG contre les trois surfaces, et pour les couleurs de série : bande de luminosité, plancher de chroma, séparation en vision déficiente protan/deutan/tritan). Ne pas substituer une valeur à l'œil.

**Surfaces & encre**

| Token | Hex | Rôle |
|---|---|---|
| `--surface-0` | `#0B0F14` | plan de page — « nuit avant l'aube » |
| `--surface-1` | `#131A22` | panneau (surface de référence des graphiques) |
| `--surface-2` | `#1B242F` | élément surélevé, ligne survolée |
| `--hairline` | `rgba(255,255,255,.07)` | liseré unique de séparation |
| `--grid` | `#1F2A36` | grille de graphique, filet |
| `--ink-1` | `#E6EDF3` | texte primaire — 14,8:1 sur `surface-1` |
| `--ink-2` | `#9FB0C0` | texte secondaire — 7,9:1 |
| `--ink-3` | `#6E8091` | libellés d'axe, atténué — 4,3:1 (à ne pas utiliser sur `surface-2`) |

**Accents & états** (usage UI : icônes, arcs, filets, texte d'état — pas comme couleur de série)

| Token | Hex | Contraste / `surface-1` | Sens, et **rien d'autre** |
|---|---|---|---|
| `--vital` | `#4FD1C5` | 9,4:1 | la mesure, le vivant, l'état sain |
| `--attention` | `#E8A33D` | 8,1:1 | ce qui mérite un regard — **pas** une alarme |
| `--grave` | `#E5534B` | 4,7:1 | état confirmé grave. Budget ≤ 1 % de la surface |
| `--good` | `#4BC97D` | 8,3:1 | confirmation d'action réussie |

**Palette de séries pour les graphiques** — ordre fixe, jamais recyclé, jamais réassigné par rang :

| Slot | Hex | Hue |
|---|---|---|
| 1 | `#2BAAA0` | cyan-teal |
| 2 | `#C98500` | ambre |
| 3 | `#3987E5` | bleu |
| 4 | `#D55181` | magenta |
| 5 | `#008300` | vert |
| 6 | `#9085E9` | violet |

> Ces six passent toutes les portes en mode sombre sur `--surface-1` (pire paire adjacente en vision déficiente ΔE 13,0 · plancher vision normale ΔE 21,0 · toutes ≥ 3:1 de contraste). Les **trois premiers** passent aussi en *toutes paires* : au-delà de trois séries dans un nuage de points ou une carte, on replie la queue en « Autre » ou on facette. Pas de 7ᵉ couleur générée.
>
> Noter que `--vital` et `--attention` sont **trop clairs pour être des couleurs de série** (hors bande de luminosité) : c'est voulu, ce sont des accents d'interface à fort contraste. Leurs équivalents série sont les slots 1 et 2.

**Séquentiel** (magnitude, heatmap) : une seule teinte, bleu, clair→foncé. Jamais d'arc-en-ciel.
**Divergent** (polarité, delta) : bleu ↔ rouge, avec un **gris neutre** au point milieu.

### 3.3 Grille & espacement

- Base **4 px**. Échelle utilisée : 4 · 8 · 12 · 16 · 24 · 32 · 48 · 64.
- Rayons : `6px` contrôles · `10px` cartes · `14px` feuilles modales · `999px` pastilles.
- Épaisseurs : filets et axes **1 px** solides, courbes de données **2 px**, marqueurs **≥ 8 px**, écart de **2 px** de la couleur de surface entre deux remplissages adjacents (jamais de bordure autour d'une marque pour la séparer).
- **Rail** 216 px · **barre supérieure** 52 px · largeur de contenu max **1440 px**, gouttière 24 px.
- Grille de contenu 12 colonnes. La vue d'ensemble : Ring sur 5 colonnes, tuiles vitales sur 7.

---

## 4. Composants signature

### 4.1 Le Health Ring — l'objet central du produit

C'est ce que l'utilisateur voit en premier, mille fois. Il porte à lui seul la thèse.

**Forme, et pourquoi :**
- **Un seul anneau, une seule valeur.** L'anneau représente la posture composite de la machine (0–100), avec le chiffre au centre en héros.
- **Décision de rigueur : pas d'arcs concentriques.** La tentation esthétique est d'empiler 4 arcs (sécurité / dérive / espace / MAJ). C'est faux : à angle égal, deux arcs de rayons différents ont des longueurs différentes, donc la comparaison visuelle est mensongère. Les quatre vitaux vivent dans des **tuiles séparées** avec des jauges linéaires (§4.2), qui, elles, sont comparables.
- Le chiffre composite est **toujours dépliable** en ses quatre composantes et en la formule exacte, au clic. Un indicateur non explicable est une décoration (§2).

**Géométrie :** diamètre extérieur 300 px · épaisseur d'arc 10 px · terminaison arrondie · départ à −90° (12 h), sens horaire. Piste : `rgba(255,255,255,.06)`.

**Couleur de l'arc, selon l'état :** sain → `--vital` · attention → `--attention` · grave → `--grave`. Un seul dégradé toléré, de la teinte à elle-même 12 % plus claire, le long de l'arc.

**La respiration :**
```
sain      : cycle 4 000 ms · opacité 0,86 → 1 · rayon du halo 8 → 14 px
attention : cycle 5 200 ms · amplitude réduite de 30 %
grave     : cycle 6 500 ms · amplitude réduite de 50 %  ← plus grave = plus lent
```
Easing sinusoïdal `ease-in-out`. L'arc lui-même ne se remplit **jamais** en boucle comme un indicateur de chargement — la longueur de l'arc est une donnée, pas une animation.

**Au centre :** le chiffre héros (Inter Tight 76, chiffres proportionnels), sous lui un libellé d'une ligne en `--ink-2` qui dit l'état en mots (« Conforme · rien n'a changé depuis 19 h »), et sous lui l'horodatage du dernier contrôle en mono 12,5.

**`prefers-reduced-motion: reduce` :** la respiration devient un halo statique à l'opacité médiane. Aucune perte d'information — la respiration est un renfort, jamais le seul porteur d'un sens.

### 4.2 Tuile vitale
Une tuile = un domaine (Sécurité, Dérive, Espace, Mises à jour).

Contenu, de haut en bas : sur-titre en `--fs-label` · valeur en `--fs-display` (chiffres proportionnels) + unité en `--ink-2` · **jauge linéaire de 4 px** ou micro-courbe de 28 px de haut avec son point final directement étiqueté · une ligne de contexte de 13 px (« 2 en dérive acceptée »).
Icône d'état 16 px **+ libellé** en haut à droite. Au survol : élévation vers `--surface-2`, 120 ms, sans déplacement.

### 4.3 Ligne de dérive (le diff)
Le composant le plus utilisé du produit. Une ligne par écart entre l'état désiré et le réel.

Structure : `[icône+libellé de gravité] [chemin de l'item en mono] [valeur voulue → valeur constatée] [source du changement] [⋯ actions]`

- La flèche `→` est en `--ink-3`. La valeur **voulue** est en `--ink-2`, la valeur **constatée** en `--ink-1` (c'est le fait, il est plus lourd).
- **La colonne « source »** est ce qui distingue Keystone : *qui* a changé ça — `Windows Update`, `Intune`, `moi (17:42)`, `inconnu`. Une dérive sans auteur identifié est le vrai signal.
- Trois actions par ligne, toujours les mêmes : **Faire converger** · **Accepter la dérive** (ouvre une note obligatoire avec raison + date d'expiration) · **Expliquer** (pourquoi cet item existe, quel risque si on le change).
- Aucune ligne ne peut être supprimée d'un clic sans confirmation ; rien n'est irréversible.

### 4.4 Carte de plan (mise à jour / convergence)
Le cœur de la confiance : **on montre tout avant de faire quoi que ce soit.**

Structure verticale : en-tête (portée, nombre d'items, durée estimée, « redémarrage requis : 1, regroupé ») → **liste par vagues ordonnées**, chaque vague pliable → bandeau du filet de sécurité (« point de restauration + export de `ubuntu-dev` avant la vague 1 ») → **liste des tests de fumée** qui jugeront le résultat → pied avec deux boutons : `Simuler` (primaire par défaut) et `Appliquer` (secondaire, verrouillé jusqu'à ce que la simulation ait tourné).

> **Le dry-run est le bouton primaire.** C'est une décision de design, pas un détail : la valeur par défaut de ce produit est de *montrer*, pas de *faire*.

### 4.5 Bandeau de rollback — la mise en scène du moment M5
Quand une mise à jour casse un test de fumée et que Keystone revient en arrière tout seul, il ne faut **pas** afficher une erreur. Il faut **raconter**, en quatre lignes et un ton posé :

> **Revenu en arrière.** La mise à jour du pilote NVIDIA 570.12 a été installée à 20:14, puis annulée à 20:19.
> *Ce qui a échoué* — le test `cuda-smoke` ne trouvait plus de GPU.
> *Ce que j'ai fait* — restauration du point pris à 20:12, pilote épinglé en 566.36 jusqu'au 1ᵉʳ septembre.
> *Où vous en êtes* — état identique à 20:12. Rien à faire de votre côté.
> `Voir le journal` `Lever l'épingle`

Traité en `--good` sur `--surface-2`, pas en rouge : c'est un **succès**, pas un incident.

### 4.6 État d'attention & bouton d'urgence
Quand un changement non expliqué est détecté, ce n'est pas un *toast* qui apparaît : c'est **toute l'interface qui change de tenue**.

- L'arc du Ring passe en `--attention`, la respiration ralentit.
- Un **filet de 2 px** apparaît en haut de la fenêtre dans la couleur d'état. C'est tout — pas de fond coloré, pas de bannière pleine largeur.
- Le rail fait remonter la section concernée en tête et l'annote (icône + libellé, jamais un chiffre nu).
- Le **bouton d'urgence**, normalement absent, apparaît en bas du rail : *Isoler la machine*. Il coupe le réseau sauf le canal d'administration, verrouille la session, capture un instantané forensique et scelle le journal. Traitement visuel : contour `--grave` 1 px sur fond transparent, **jamais un bloc rouge plein**. Il exige une confirmation par Windows Hello — parce que son coût est réel.

### 4.7 Palette de commandes (⌘K / Ctrl+K)
**Parité totale avec la CLI** : tout ce que la CLI sait faire est atteignable ici, et chaque résultat affiche **la commande CLI équivalente** en mono à droite. L'utilisateur apprend son propre outil en s'en servant.
Trois sections : Actions · Items (dérives, apps, distros, volumes) · Documentation. Navigation clavier complète, `Esc` ferme, aucune souris nécessaire.

### 4.8 Rapport matinal
Une carte unique, lisible en 15 secondes, qui répond à une seule question : **qu'est-ce qui a changé depuis hier ?** En trois blocs — *ce qui a changé* / *ce qui vous attend* / *ce que j'ai fait pendant la nuit*. Révélation en cascade de 60 ms, une seule fois par jour. Si rien n'a changé, la carte le dit en une ligne et se tait.

### 4.9 Timeline forensique
Bandeau horizontal zoomable, échelle temporelle en mono. Une marque par événement, coloré par catégorie via la palette de séries (§3.2) avec **légende toujours présente**. Survol → infobulle ; focus clavier → même contenu. **Un onglet « Tableau » donne l'équivalent textuel intégral** : aucune valeur n'est accessible uniquement par le survol.

---

## 5. Les huit moments de vérité

Chacun doit être maquetté comme un écran ou un état à part entière. C'est ici que se gagne l'attachement.

| # | Moment | Émotion visée | Le geste de design |
|---|---|---|---|
| **M1** | **Premier lancement** | révélation | Keystone ne demande **aucune configuration**. Il scanne en lecture seule et montre la machine comme l'utilisateur ne l'a jamais vue, puis propose : « j'ai écrit votre état actuel dans `workstation.yaml` — 312 items. Rien n'a été modifié. » On **importe** le réel, on ne fait pas remplir un formulaire. |
| **M2** | **Le matin** | contrôle tranquille | §4.8. 15 secondes, une carte, puis silence. |
| **M3** | **Dérive détectée** | curiosité, pas culpabilité | Le mot est « écart », jamais « problème ». La colonne « source » est la première chose lisible. |
| **M4** | **Premier converge** | confiance | Simulation obligatoire, diff intégral, filet de sécurité annoncé, puis l'anneau retrouve son rythme de 4 s. La récompense est le retour du calme. |
| **M5** | **La MAJ casse, rollback auto** | soulagement | §4.5. **C'est le moment qui crée la loyauté** : traité en succès, raconté en quatre lignes. |
| **M6** | **Espace récupéré** | satisfaction tangible | Un compteur qui **décroît** en 900 ms jusqu'au chiffre final, l'octet exact, et la mention « en quarantaine 30 jours, restaurable » — on célèbre la réversibilité autant que le gain. |
| **M7** | **Signal d'intrusion** | gravité sans hystérie | §4.6. Toute l'interface change de tenue, rien ne crie. Un fait, une heure, une conséquence, une action. |
| **M8** | **Reconstruction après sinistre** | dignité | Un écran plein qui montre la machine se reconstituer domaine par domaine depuis le dépôt git, avec le temps restant. À la fin : « votre poste est revenu. » Rien d'autre. |

---

## 6. Accessibilité — exigences non dismissables

- **WCAG 2.2 niveau AA** partout. Texte normal ≥ 4,5:1, texte large et éléments graphiques porteurs de sens ≥ 3:1. Les valeurs du §3.2 sont déjà vérifiées : ne pas les modifier sans recalculer.
- **Jamais de sens porté par la couleur seule.** Tout état = **icône + libellé texte** (`Conforme` / `Attention` / `Grave`). Un dashboard de santé rouge-vert est illisible pour ~8 % des hommes ; ici la couleur est le troisième porteur, après le mot et la forme.
- **Navigation clavier intégrale**, ordre de tabulation logique, anneau de focus visible 2 px `--vital` avec 2 px de décalage. Aucune fonction inatteignable au clavier.
- **`prefers-reduced-motion: reduce`** : la respiration se fige, les cascades deviennent des apparitions immédiates, les compteurs affichent directement leur valeur finale. Aucune information perdue.
- **`forced-colors: active`** (mode contraste Windows) : géré explicitement, avec la trame de texture 45°/135° en remplacement de la couleur sur les remplissages de données.
- **Lecteur d'écran** : chaque graphique possède un équivalent tableau ; le Ring expose une valeur `role="meter"` avec `aria-valuetext` en français lisible (« posture 94 sur 100, conforme »).
- **Cible de pointage ≥ 24 px** sur tout élément interactif, y compris les marques de la timeline.
- **Français et anglais** dès le départ ; aucune chaîne codée en dur, formats de date et d'unité localisés.

---

## 7. Écrans à produire

| # | Écran | Contenu | Priorité |
|---|---|---|---|
| **E1** | Vue d'ensemble — état **sain** | Ring, 4 tuiles vitales, résumé de dérive, prochaine fenêtre de maintenance, bandeau timeline | ★★★ |
| **E2** | Vue d'ensemble — état **attention** | même écran, tenue d'attention (§4.6), bouton d'urgence présent | ★★★ |
| **E3** | Dérive | tableau de lignes de dérive (§4.3), filtres en une rangée au-dessus, panneau latéral d'explication | ★★★ |
| **E4** | Mises à jour | carte de plan (§4.4) avec vagues, rings, tests de fumée, filet de sécurité ; état post-rollback (§4.5) | ★★★ |
| **E5** | Espace | attribution empilée par consommateur avec légende + onglet tableau ; liste de quarantaine avec TTL | ★★ |
| **E6** | Sécurité & posture | posture dépliée en composantes, diffs de persistance / certificats / ports, coexistence Intune | ★★ |
| **E7** | Premier lancement (M1) + Rapport matinal (M2) | les deux moments fondateurs | ★★★ |
| E8 | WSL & VM · Journal · Sauvegarde | secondaires, à cadrer après validation des 7 premiers | ★ |

**Livrables attendus par écran :** version 1440 px et version 1024 px · état normal, survol, focus clavier, chargement, vide, erreur · variante `reduced-motion`.

---

## 8. Prompt prêt à coller dans Claude Design

> Conçois **Keystone**, un plan de contrôle pour poste de travail d'ingénieur (Windows 11 + WSL2 + VM Hyper-V) : il surveille l'état de la machine, détecte les écarts avec un état désiré déclaré, orchestre les mises à jour avec rollback automatique, et signale tout changement non expliqué.
>
> **Direction : « poste de pilotage ».** Émotion visée : la maîtrise et le calme sous pression. La thèse du produit est *« le calme est la fonctionnalité »* — la récompense offerte à l'utilisateur est un écran silencieux, pas une stimulation. Interdits absolus : dark patterns de peur, rouge en aplat, clignotement, gamification, badges de non-lus, émoji.
>
> **Mode sombre natif.** Surfaces `#0B0F14` / `#131A22` / `#1B242F`, liseré unique `rgba(255,255,255,.07)`, profondeur par la lumière et non par l'ombre, grain monochrome à 2 % en overlay global. Encre `#E6EDF3` / `#9FB0C0` / `#6E8091`. Accents : `#4FD1C5` (vital), `#E8A33D` (attention), `#E5534B` (grave, ≤ 1 % de la surface), `#4BC97D` (succès). Couleurs de série pour les graphiques, dans cet ordre fixe : `#2BAAA0`, `#C98500`, `#3987E5`, `#D55181`, `#008300`, `#9085E9`.
>
> **Typo :** Inter Tight (titres et chiffre héros, chiffres proportionnels), Inter (corps), JetBrains Mono avec `tabular-nums` pour toute la télémétrie, les tables et les chemins.
>
> **Élément central : le « Health Ring ».** Un anneau unique de 300 px, épaisseur 10 px, une seule valeur composite avec le chiffre en héros au centre. Il **respire** à 0,25 Hz (cycle de 4 s, opacité 0,86→1, halo 8→14 px). Quand l'état se dégrade, la respiration **ralentit** (6,5 s) au lieu de s'accélérer — un instrument qui panique fait paniquer son pilote. Pas d'arcs concentriques : les quatre vitaux sont des tuiles séparées à jauge linéaire, parce que des arcs de rayons différents ne sont pas comparables.
>
> **Timings :** 120 ms micro-feedback, 200 ms transitions, 400 ms apparitions, easing `cubic-bezier(.2,.8,.2,1)`. Une animation couvre une latence, ne l'ajoute jamais.
>
> **Ton des textes :** factuel, calme, sans reproche, jamais de majuscules criées ni de point d'exclamation, le chiffre avant l'adjectif, jamais de code d'erreur nu. Exemple de la voix juste : « Une autorité de certification racine a été ajoutée hier à 23:14. Je ne la reconnais pas. Tant qu'elle est présente, votre trafic HTTPS peut être lu. »
>
> **Écrans :** (1) vue d'ensemble état sain, (2) vue d'ensemble état attention avec bouton *Isoler la machine* en contour et non en aplat, (3) table de dérive avec une colonne « source du changement » et trois actions par ligne — Faire converger / Accepter la dérive / Expliquer, (4) plan de mise à jour par vagues avec tests de fumée et bouton *Simuler* en primaire et *Appliquer* verrouillé jusqu'à simulation, plus l'état « revenu en arrière » traité en succès vert et non en erreur, (5) attribution de l'espace disque avec légende et onglet tableau, (6) posture de sécurité dépliée, (7) premier lancement — l'outil n'exige aucune configuration, il scanne et propose d'importer l'état existant.
>
> **Accessibilité :** WCAG 2.2 AA, jamais de sens porté par la couleur seule (icône + libellé partout), navigation clavier intégrale avec anneau de focus 2 px, `prefers-reduced-motion` qui fige la respiration sans perte d'information, tout graphique doublé d'un équivalent tableau, cibles ≥ 24 px.

---

## 9. Ce qui reste à trancher

1. **Icônes** — jeu sur mesure (24 px, trait 1,5 px, grille 24) ou base Lucide retracée ? Un cockpit mérite ses propres pictogrammes, mais c'est 3 semaines de travail.
2. **Son** — trois échantillons seulement (convergence réussie · rollback effectué · attention), sub-bass + bois, **opt-in** et muet en mode Réunion. À valider : le son est-il un atout ou un passif sur un poste professionnel ?
3. **Densité** — proposer un mode « dense » (interlignes réduits, plus de lignes visibles) pour la vue Dérive quand elle dépasse 40 items ?
4. **Widget de bureau / barre des tâches** — un anneau miniature dans la zone de notification, ou rien du tout ? La thèse du silence plaide pour « rien », mais un état à un coup d'œil a de la valeur.
5. **Vue mobile en lecture seule** — consulter l'état de son poste depuis son téléphone, avec un unique bouton d'action : *Isoler la machine*. À maquetter en phase 2.
