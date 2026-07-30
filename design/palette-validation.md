# Validation de la palette

> **Les couleurs de `tokens.css` ne se choisissent pas à l'œil.** Elles se
> vérifient par calcul, et cette note enregistre le résultat. Si une teinte doit
> changer, recalculer et mettre à jour ce fichier **dans le même commit**.

Date de la validation : **30 juillet 2026**
Surfaces de référence : `--surface-0 #0B0F14` · `--surface-1 #131A22` · `--surface-2 #1B242F`

---

## 1. Contraste de l'encre et des accents (WCAG 2.2)

Rapports de contraste calculés selon la formule de luminance relative WCAG.

| Token | Hex | / `surface-0` | / `surface-1` | / `surface-2` | Verdict |
|---|---|---|---|---|---|
| `--ink-1` | `#E6EDF3` | 16,27 | 14,83 | 13,26 | ✅ AAA partout |
| `--ink-2` | `#9FB0C0` | 8,64 | 7,88 | 7,05 | ✅ AAA partout |
| `--ink-3` | `#6E8091` | 4,72 | 4,30 | 3,85 | ⚠️ voir note |
| `--vital` | `#4FD1C5` | 10,30 | 9,39 | 8,40 | ✅ |
| `--attention` | `#E8A33D` | 8,91 | 8,12 | 7,27 | ✅ |
| `--grave` | `#E5534B` | 5,19 | 4,73 | 4,23 | ✅ AA texte normal |
| `--good` | `#4BC97D` | 9,11 | 8,30 | 7,43 | ✅ |
| `--grid` | `#1F2A36` | 1,32 | 1,20 | 1,08 | ✅ attendu — récessif par conception |

**Note sur `--ink-3`.** À 3,85:1 sur `--surface-2`, il passe sous le seuil AA de
4,5:1 pour du texte normal. Règle d'usage : **`--ink-3` ne s'emploie que sur
`--surface-0` et `--surface-1`**, où il tient (4,72 et 4,30). Sur une ligne
survolée qui passe en `--surface-2`, le texte atténué remonte à `--ink-2`.

C'est le genre de contrainte qu'on découvre par le calcul et jamais à l'œil — sur
un fond sombre, `#6E8091` *paraît* parfaitement lisible partout.

---

## 2. Palette de séries — les six portes

Validation en mode sombre, surface `#131A22`, sur la liste de paires **adjacentes**
(barres, aires empilées, lignes).

```
Palette (dark, surface #131A22, categorical): 6 slots
  [PASS] Lightness band     all 6 inside L 0.48–0.67
  [PASS] Chroma floor       all 6 >= 0.1
  [PASS] CVD separation     worst adjacent #008300↔#D55181 ΔE 13.0 (deutan) · tritan 15.3
  [PASS] Normal-vision floor worst adjacent #C98500↔#2BAAA0 ΔE 21.0 (normal)
  [PASS] Contrast vs surface all 6 >= 3:1
  → ALL CHECKS PASS
```

Ordre validé, à ne pas permuter sans revalider :

| Slot | Hex | Teinte |
|---|---|---|
| `--s1` | `#2BAAA0` | cyan-teal |
| `--s2` | `#C98500` | ambre |
| `--s3` | `#3987E5` | bleu |
| `--s4` | `#D55181` | magenta |
| `--s5` | `#008300` | vert |
| `--s6` | `#9085E9` | violet |

### Le cas « toutes paires »

Pour les formes où **toute** paire peut se retrouver côte à côte — nuage de points,
bulles, carte choroplèthe, petits multiples — la liste de paires adjacentes ne
suffit pas. Les **trois premiers slots** passent la validation en toutes paires :

```
Palette (dark, surface #131A22, categorical): 3 slots
  [PASS] CVD separation      worst all-pairs #C98500↔#2BAAA0 ΔE 15.1 (protan)
  [PASS] Normal-vision floor worst all-pairs #3987E5↔#2BAAA0 ΔE 16.4 (normal)
  → ALL CHECKS PASS
```

**Conséquence pratique :** au-delà de trois séries dans ces formes, replier la queue
en « Autre » ou facetter en petits multiples. Pas de 7ᵉ couleur générée, jamais.

---

## 3. Pourquoi `--vital` et `--attention` ne sont pas des couleurs de série

Elles échouent délibérément la bande de luminosité :

```
Palette (dark, surface #131A22, categorical): 3 slots
  [FAIL] Lightness band  outside band: [["#4FD1C5",0.786],["#E8A33D",0.765]]
```

L = 0,786 et 0,765, au-delà du plafond de 0,67 attendu pour une couleur de série.
C'est **voulu** : ce sont des accents d'interface, qui doivent porter du texte et
des icônes avec un contraste élevé (9,4:1 et 8,1:1). Une couleur de série, elle,
remplit des surfaces et doit rester dans une bande étroite pour que les séries
soient comparables entre elles.

Leurs équivalents série sont les slots 1 (`#2BAAA0`) et 2 (`#C98500`) — la même
famille de teintes, redescendue dans la bande.

**Ne jamais utiliser `--vital` ou `--attention` comme couleur de remplissage dans
un graphique.** C'est l'erreur la plus facile à commettre avec cette palette.

---

## 4. Règles d'encodage qui accompagnent la palette

Une palette validée ne suffit pas ; l'usage compte autant.

| Règle | Pourquoi |
|---|---|
| **Jamais de sens porté par la couleur seule** — icône **+** libellé, systématiquement | ~8 % des hommes ont une vision déficiente des couleurs. Un tableau de bord de santé rouge/vert leur est illisible. |
| **La couleur suit l'entité, jamais son rang** | Un filtre qui change le nombre de séries ne doit pas repeindre les survivantes : un lecteur qui a appris « Docker est cyan » serait trahi. |
| **Séquentiel = une seule teinte**, clair→foncé | Un arc-en-ciel n'a pas d'ordre perceptif. |
| **Divergent = deux teintes opposées + gris neutre au milieu** | Le point milieu doit se lire comme « rien ». |
| **Les couleurs d'état sont réservées** et ne servent jamais de série | Sinon une couleur d'état se fait passer pour une donnée. |
| **Écart de 2 px de la couleur de surface** entre deux remplissages | Jamais de bordure dessinée autour d'une marque pour la séparer. |
| **Le reste / « Autre » prend un gris neutre** (`--s-other`) | Ce n'est pas une catégorie d'intérêt ; en teinte de série, il domine le graphique sans rien dire. |
| **Tout graphique possède un équivalent tableau** | L'infobulle enrichit, elle ne conditionne jamais l'accès à une valeur. |

---

## 5. Décisions de forme prises contre l'esthétique

Deux cas où la tentation visuelle était trompeuse, notés ici parce qu'ils
reviendront à chaque nouvelle vue.

### Le Health Ring n'a pas d'arcs concentriques

La tentation : empiler quatre arcs (sécurité / dérive / espace / mises à jour) dans
un anneau unique. C'est joli, et **c'est faux** : à angle égal, deux arcs de rayons
différents ont des longueurs différentes, donc la comparaison visuelle ment.

Le choix retenu : **un anneau, une seule valeur composite**, avec le chiffre en
héros au centre — et les quatre vitaux en tuiles séparées avec des jauges
**linéaires**, qui, elles, sont comparables.

### La tuile de dérive n'a pas de jauge

3 écarts sur 312 items donnerait un remplissage de 1 % : soit invisible, soit
exagéré pour être visible — donc mensonger. Le choix retenu : **trois comptes
étiquetés directement** (309 conformes · 2 acceptées · 1 active).

Une jauge n'est justifiée que lorsque le ratio a un sens visuel. Sinon, le nombre
*est* le graphique.

---

## 6. Reproduire la validation

Les résultats ci-dessus ont été produits avec le validateur de palette de la
méthode de visualisation de données (bande de luminosité, plancher de chroma,
séparation en vision déficiente en OKLab ×100, contraste contre surface), et une
implémentation directe de la formule de contraste WCAG pour le tableau de la
section 1.

Seuils appliqués :

| Contrôle | Seuil |
|---|---|
| Bande de luminosité (série, mode sombre) | L ∈ [0,48 ; 0,67] |
| Plancher de chroma | ≥ 0,1 |
| Séparation en vision déficiente | ΔE ≥ 8 (cible) · 6–8 toléré **seulement** avec encodage secondaire |
| Plancher en vision normale | ΔE ≥ 15 — **échec strict** en dessous |
| Contraste série / surface | ≥ 3:1 |
| Contraste texte normal | ≥ 4,5:1 (AA) |

**À refaire à chaque changement de teinte ou de surface.** Un contraste calculé
contre une surface qui a bougé ne veut plus rien dire.
