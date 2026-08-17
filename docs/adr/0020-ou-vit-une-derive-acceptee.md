# ADR-0020 — L'acceptation vit dans le fichier, la décision vit dans le journal

- **Statut** : Accepté le 2026-08-17, **amendé le même jour sur sa décision
  n° 2** (voir la section « Amendement » en fin de document). Le corps du texte
  est conservé tel qu'il a été écrit avant toute ligne de code : c'est lui qui
  porte le raisonnement, et le réécrire pour qu'il tombe juste après coup
  détruirait la seule chose qu'une ADR apporte.
- **Date** : 2026-08-17
- **Exigences concernées** : D2-06, D2-07, D2-01, P2, P3, P6
- **Précise** : ADR-0016 (la lecture du fichier), ADR-0014 (ce que le journal a
  le droit de porter), et ADR-0017 (ce que Keystone écrit, et ce qu'il se
  contente de proposer). *Cette ligne annonçait, avant l'amendement, que
  `ks accept` n'écrivait pas le fichier de l'utilisateur ; elle est corrigée ici
  plutôt que laissée en place, une ADR dont l'en-tête contredit son propre
  contenu étant pire qu'une ADR absente.*

## Contexte

La dérive acceptée est le seul mécanisme qui empêche un fichier d'état de
pourrir sous une couche d'exceptions dont plus personne ne connaît le motif
(risque R4). Le dépôt en porte déjà la moitié. C'est l'autre moitié, et le
départ entre les deux, que ce document tranche.

### Ce qui existe, mesuré le 2026-08-17

Du côté du modèle, tout est là. `ks_core::DriftStatus::Accepted` porte sa
raison, son échéance et son décideur ; `is_convergeable` refuse de converger un
écart toléré non échu ; `is_expired_acceptance` dit qu'une tolérance échue
redevient un écart ; `DriftSummary::build` compte déjà l'expiration parmi les
écarts actifs, et un test le vérifie. Du côté du fichier, `EcartAccepte` se lit
depuis `workstation.yaml`, ses cinq champs sont obligatoires par le typage, et
`un_ecart_accepte_exige_sa_raison_et_son_echeance` éprouve le refus en retirant
tour à tour `reason` et `expires`.

Et pourtant, `crates/ks-cli/src/confrontation.rs` ne lit jamais le champ
`accepted_drift` : le mot n'y figure que dans une chaîne de test, comme point
d'ancrage d'une substitution textuelle. La liste est lue, validée par le typage,
puis abandonnée sans qu'aucun calcul la regarde. Un poste dont le
fichier tolère explicitement un écart obtient exactement le même `ks diff` qu'un
poste dont le fichier ne tolère rien. La déclaration rassure et n'agit pas,
c'est-à-dire le pire des trois états possibles, celui-là même que l'ADR-0008 a
corrigé pour les items illisibles comptés parmi les conformes.

`crates/ks-cli/src/emetteur.rs` écrit, de son côté, la ligne `acceptedDrift: []`
en dur, avec ce commentaire : « c'est là que s'écrivent la raison et l'échéance
qu'exige D2-06 ». La place est réservée ; personne ne s'y assied.

### Trois écarts découverts en mesurant, et qu'il faut trancher ici

**Une raison vide passe.** Le typage exige le champ, jamais son contenu :
`reason: ""` est un `String` parfaitement valide, et le fichier est accepté. Or
une acceptation sans motif lisible est exactement l'exception permanente
silencieuse que D2-06 interdit en toutes lettres. Le test cité plus haut éprouve
l'absence du champ, pas sa vacuité, et le second test qui vérifie
`!reason.is_empty()` ne porte que sur l'exemple versionné.

**Deux acceptations peuvent viser le même chemin.** `acceptedDrift` est une
liste, pas une table. Le lecteur refuse une clé écrite deux fois dans `desired`,
et l'ADR-0016 explique pourquoi : « une exception ajoutée en haut, oubliée,
écrasée en bas » est le mode de pourrissement classique d'un fichier de
configuration. Ce refus, la liste ne l'a pas. Deux entrées pour
`security.services.fax.startup`, l'une échue et l'autre valable trois ans, ne
posent aujourd'hui aucune question à personne, et aucun document ne dit laquelle
l'emporterait.

**`ks import --force` détruit les acceptations sans un mot.** L'émetteur ne
réécrit pas le fichier : il en fabrique un neuf depuis un scan. Il sait produire
`apiVersion`, `kind`, `metadata.name` et la table `desired`. Il ne sait produire
ni `inherits`, ni `owner`, ni `description`, ni la moindre acceptation, puisqu'il
n'en reçoit aucune. Un `ks import --force` sur un fichier vivant emporte donc
quatre catégories d'écriture humaine, de façon irréversible et silencieuse,
c'est-à-dire contre le principe P3 que la commande croit respecter en refusant
d'écraser sans drapeau.

### Le piège que la mise en œuvre naïve rencontrerait

L'écriture d'une acceptation dans le fichier semble être le geste évident de
`ks accept`. Elle ne l'est pas : l'émetteur produit un document neuf, jamais une
modification. Lui confier la mise à jour reviendrait à réécrire intégralement un
fichier que l'utilisateur a rédigé, commenté et ordonné à la main, et à perdre
les quatre catégories ci-dessus par la même occasion. La perte serait
irréversible, donc inadmissible au sens de P3, qui fait de la réversibilité un
critère d'admission et non une qualité souhaitable.

### La frontière de la lecture seule, telle qu'elle est réellement

La règle du projet dit que les phases 0 et 1 n'écrivent rien du tout. Prise à la
lettre, elle est déjà fausse : `ks scan --record` écrit le magasin, `ks report`
écrit un rapport, `ks import` écrit le yaml. Chacune de ces trois écritures a été
justifiée séparément, dans trois endroits différents, et aucune ne dit où passe
la ligne. Un lot qui ajoute une commande a besoin de cette ligne, faute de quoi
elle se déplacera d'un raisonnement local à l'autre : « on écrit déjà le journal,
alors une petite modification du yaml… ».

### Ce que P2 exige, et ce qu'il n'exige pas

Il n'existe volontairement pas de drapeau `--dry-run` dans ce produit, seulement
`--apply`, pour qu'on ne puisse pas écrire par omission. La question posée à ce
lot est donc double : `ks accept` doit-il simuler avant d'agir, et une entrée de
journal est-elle une écriture au sens où P2 l'entend ?

## Décision

**1. Le fichier porte l'état, le journal porte la décision, et ils ne se
contredisent jamais parce qu'ils ne répondent pas à la même question.**

`workstation.yaml` répond à « quelles tolérances sont en vigueur aujourd'hui ».
C'est la seule source que `ks diff` consulte, et D2-01 en fait la source de
vérité unique, versionnée, relisible en revue. Le journal répond à « qui a décidé
quoi, quand, et pour quel motif ». Il n'est jamais lu pour calculer un verdict.

Aucune priorité n'est donc à arbitrer, et c'est le point : une règle de priorité
suppose deux réponses concurrentes à la même question, ce que cette répartition
rend impossible. Une acceptation écrite à la main prend effet sans passer par le
journal ; une décision journalisée puis jamais recopiée dans le fichier reste
sans effet, et les deux énoncés sont vrais en même temps sans se heurter.

Le motif décisif est la reconstructibilité. Le magasin est local, effaçable,
absent d'un dépôt git et absent d'une machine reconstruite. Si la politique du
poste y vivait, `workstation.yaml` ne décrirait plus qu'une moitié de ce que
l'utilisateur veut, et une réinstallation ferait réapparaître des écarts sans
explication. Le fichier doit se suffire.

**2. Keystone n'édite jamais un fichier qu'il n'a pas intégralement produit.**

C'est la ligne que le contexte réclame, et elle se dit en trois cercles :

| Cercle | Règle | Portée |
|---|---|---|
| Le système | rien, jamais, jusqu'à la Phase 2 | registre, services, politiques, fichiers d'autrui |
| Les données de Keystone | sur demande explicite, jamais par omission | `%LOCALAPPDATA%\Keystone\journal.sqlite` |
| Les fichiers de l'utilisateur | Keystone **crée** un document entier, il ne **modifie** aucun document existant | `workstation.yaml`, le rapport HTML |

La formule « rien du tout » est donc abandonnée, parce qu'elle est fausse depuis
la Phase 0.5 et qu'une règle fausse ne protège plus rien. La formule retenue
distingue trois gestes que la précédente confondait, et elle explique pourquoi
`ks import --force` reste permis, remplaçant un document que Keystone produit
intégralement, tandis que `ks accept` ne l'est pas, modifiant un document dont
l'humain est l'auteur.

**3. `ks accept` n'écrit pas le fichier : il imprime le bloc à coller et
journalise la décision.**

```
ks accept security.services.fax.startup \
  --reason "requis par le pilote du scanner du labo" --expires 2026-10-15
```

La commande scanne, charge le fichier, confronte, vérifie que le chemin désigne
bien un item observé et en écart, journalise la décision, puis affiche le bloc
prêt à coller sous `acceptedDrift`, guillemets et indentation compris. Le geste
est exactement celui que l'ADR-0017 a retenu pour git : le service est rendu, la
porte reste fermée.

Le mode de défaillance décide autant que le principe. Une acceptation qu'on
oublie de coller laisse l'écart visible ; une insertion automatique qui se
tromperait de ligne ferait disparaître un écart ou corromprait la source de
vérité du poste. Entre deux erreurs possibles, on choisit celle qui montre plus
que prévu.

**4. Il n'y a pas d'`--apply` sur `ks accept`, et ce n'est pas une entorse à P2.**

`--apply` qualifie l'écriture sur la machine. Il ne qualifie pas l'écriture du
journal de Keystone, sans quoi `ks scan --record` en porterait un. Une commande
qui ne touche ni au système ni au fichier de l'utilisateur n'a rien à appliquer,
et lui coller le drapeau par symétrie le banaliserait précisément là où il doit
rester rare et sérieux.

P2 est tenu, et il l'est au sens fort : la sortie de `ks accept` **est** le diff.
Elle montre l'écart tel qu'il est mesuré (valeur déclarée, valeur constatée), le
texte exact qui entrera dans le fichier, et la durée de la tolérance. Ce que
l'utilisateur voit avant de coller est, mot pour mot, ce qu'il obtiendra.

L'écriture au journal, elle, a lieu sans drapeau, et c'est délibéré : le nom de
la commande est le consentement. `ks scan` annonce une lecture, donc sa
journalisation se demande ; `ks accept` annonce une décision, et une décision qui
ne laisserait pas de trace ne serait pas une décision, ce serait un affichage.

**5. La décision est une entrée du journal chaîné, pas une table de plus.**

L'ADR-0014 a écarté le journal pour les valeurs observées, et sa table de
propriétés dit exactement pourquoi : « une entrée par **décision** ». Ce lot est
le cas pour lequel le journal existe. Une décision n'est jamais purgée, ce qui
interdit la rétention et rejoint le chaînage ; elle se lit une par une, par un
humain ; et elle doit être indétachable de son motif, faute de quoi on réécrit
après coup la raison ou l'échéance d'une tolérance, c'est-à-dire le pourrissement
que D2-06 et D2-07 existent pour empêcher. Une seconde table ne serait pas
chaînée, ou porterait un second chaînage à maintenir : deux mécanismes de preuve
dérivent, et celui qu'on regarde le moins ment le premier.

L'entrée porte : l'instant (`at`, qui est le `decidedAt` du bloc), l'acteur
(`Actor::Human`, qui est le `decidedBy`), le chemin de l'item en `target`, le
diff accepté en `diff`, et un résultat nouveau, `Outcome::Decided { reason,
expires }`.

Cette variante est nécessaire, et aucune des six existantes ne convient :
`Applied` laisserait croire à une écriture, `Simulated` à une action envisagée
puis retenue, `Observed` à un relevé. Aucune n'est vraie, et un journal qui se
trompe sur la nature de ce qu'il consigne perd sa raison d'être. Elle porte ses
deux champs plutôt que de les laisser à un texte libre, pour la raison écrite
dans les conventions Rust du projet : une décision sans raison ni échéance doit
être inconstructible, et non refusée à l'exécution.

**Le matériau d'empreinte couvre les deux champs.** Il écrit aujourd'hui
l'étiquette du résultat puis son détail ; il en écrira deux pour cette variante.
L'arité variable n'ouvre aucune ambiguïté : l'étiquette précède les champs,
appartient à un ensemble fermé, et chaque champ reste préfixé de sa longueur. Les
entrées déjà écrites produisent le même matériau qu'avant, donc restent
vérifiables. Sans cette couverture, on modifierait après coup l'échéance d'une
tolérance sans casser la chaîne, ce qui est le défaut que l'ADR-0004 a corrigé
pour `diff` et `outcome`.

`verb` vaut `"accept"`. C'est une étiquette d'entrée de journal, comme `"scan"` :
**ce lot n'ajoute aucun verbe à `ks-broker`, ne touche pas au broker, et n'écrit
rien sur la machine.** Les quatre questions du modèle de menace ne s'appliquent
donc pas ici, et l'écrire évite qu'un relecteur les cherche.

La consultation ne demande pas de commande nouvelle : les décisions sont des
entrées, `ks journal` les affiche, et un filtre `--verb accept` isole celles-ci.
Une commande dédiée serait une seconde façon de lire le même registre.

**6. Un écart accepté reste un écart. Il change de compte, jamais de verdict.**

`Verdict` garde ses quatre valeurs et `verdict_publie` ne bouge pas : un item
toléré est publié `Ecart`, parce que la valeur constatée diffère bien de la
valeur déclarée. Le verdict dit ce que la comparaison a trouvé, un fait ; le
statut dit ce qu'on en fait, une politique. Les confondre effacerait le fait
derrière la politique, et c'est la correction que l'ADR-0008 a déjà imposée pour
les items illisibles.

L'affichage sépare donc deux listes, et jamais une seule :

```
  écarts actifs      3
  écarts tolérés     1     échéance la plus proche : 2026-10-15, dans 59 jours
  conformes          31
```

L'item toléré est nommé, avec sa raison, son échéance et le nombre de jours qui
restent. La barrière est un invariant de couverture : la somme des non
contraints, des conformes, des écarts actifs, des écarts tolérés et des
incomparables reste égale au nombre d'items observés. Un item toléré ne peut donc
ni disparaître, ni glisser vers les conformes, sans que le total cesse de tomber
juste.

**7. L'expiration n'est pas un événement : c'est une propriété de la comparaison
à l'instant où elle a lieu.**

Il n'y a rien à ordonnancer, aucune tâche planifiée, aucun balayage, aucun état à
faire vieillir. Une acceptation échue n'est simplement plus lue comme valable au
scan suivant, et l'écart redevient actif tout seul, ce que D2-06 exige. Le temps
qui passe suffit, ce qui rend le mécanisme inratable.

Trois précisions le rendent testable. L'échéance est une **date civile** et non
un instant : `DriftStatus::Accepted::expires` passe de `Timestamp` à
`chrono::NaiveDate`, ce que le fichier écrit déjà, parce qu'une conversion
arbitraire vers un instant (à quelle heure, dans quel fuseau ?) finirait par
diverger d'un site d'appel à l'autre. Elle est **inclusive** : `expires:
2026-10-15` couvre le 15 et l'écart redevient actif le 16, la lecture qui ne
surprend jamais dans le mauvais sens, une tolérance durant au pire un jour de
plus que prévu et jamais un jour de moins. Et la date du jour est un
**paramètre** des fonctions qui comparent, jamais une lecture d'horloge interne :
une horloge dans un chemin asserté ne se teste pas deux fois de la même façon.

**8. Les cas limites, et le seul qui refuse le document.**

| Cas | Ce que fait Keystone |
|---|---|
| Le chemin visé n'est porté par aucun item observé | il rejoint les « déclarés, non observés » de l'ADR-0010, avec ses deux causes nommées (faute de frappe, ou item légitimement disparu). Aucun mécanisme nouveau |
| L'item est observé mais n'est pas en écart | ligne d'information : « tolérance sans objet, l'item est conforme ; cette ligne peut être retirée ». Jamais bloquant |
| L'échéance est déjà passée | l'écart est actif, et la ligne le dit : « échue depuis 12 jours, cette ligne peut être retirée » |
| L'échéance est très lointaine | acceptée, mais la **durée** est toujours affichée à côté de la date. D2-06 proscrit l'exception permanente *silencieuse* : « dans 26 ans » ne l'est pas |
| Une raison vide ou blanche | **le document est refusé**, comme il l'est déjà pour une clé en double |
| Deux acceptations pour le même chemin | **le document est refusé**, en nommant le chemin |

Les deux refus visent le même défaut : une tolérance dont le motif ou la portée
est indéterminé. Les quatre autres cas ne refusent rien, et la différence n'est
pas de sévérité mais de nature. Une déclaration mal typée empêche de comparer
l'item, donc refuse le fichier (ADR-0016) ; une acceptation devenue sans objet
n'empêche rien du tout, et bloquer un utilisateur sur un fichier qui décrit
correctement son intention serait disproportionné. Ces lignes sont affichées sans
reproche : « échue depuis 12 jours » et non « vous avez oublié ».

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| **Les acceptations vivent dans le magasin, le yaml ne les porte plus** | D2-01 deviendrait faux, l'exemple versionné mentirait, et la politique du poste ne survivrait ni à une réinstallation ni à un clone du dépôt. Une tolérance disparue avec `%LOCALAPPDATA%` ferait réapparaître des écarts sans explication |
| **Les deux portent l'acceptation, le magasin fait foi** | le fichier deviendrait un reflet, donc une source de vérité en apparence seulement. C'est la configuration qui pourrit le plus vite : celle qu'on lit en revue de code sans qu'elle décide de rien |
| **Les deux portent l'acceptation, le fichier fait foi, et l'écart se réconcilie** | il faudrait afficher, à chaque `ks diff`, les tolérances du fichier sans trace de décision. En Phase 1 l'utilisateur écrit à la main : ce serait donc un avertissement permanent sur un usage légitime, c'est-à-dire du bruit, et un reproche. Le désaccord n'est pas une anomalie à signaler mais une conséquence assumée du point n° 1 |
| **`ks accept` réécrit le fichier avec l'émetteur** | mesuré : l'émetteur produit un document neuf depuis un scan, il ne sait produire ni `inherits`, ni `owner`, ni `description`, ni les acceptations existantes, ni un seul commentaire écrit à la main. La réécriture perdrait donc du **contenu**, pas seulement de la mise en forme, et cette perte est irréversible (P3) |
| **`ks accept` insère chirurgicalement, en préservant le reste octet pour octet, avec sauvegarde préalable** | c'est l'option sérieuse, et elle coûte un éditeur YAML positionnel maison : trouver la clé, distinguer `acceptedDrift: []` d'une liste garnie, gérer son absence, l'indentation, les commentaires intercalaires, CRLF contre LF, la marque d'ordre des octets. Un éditeur positionnel qui se trompe écrit un fichier corrompu, et ce fichier est la source de vérité du poste. La sauvegarde déposée à côté ajoute par ailleurs un fichier non demandé dans le dépôt de l'utilisateur, que le premier `git add -A` emporterait. Le coût est réel et permanent, le bénéfice est d'éviter un collage. Reste atteignable en Phase 2, quand le moteur d'instantanés donnera un vrai retour arrière |
| **Un drapeau `--write` sur `ks accept`, explicite** | déplace la question sans la trancher : le jour où le drapeau est passé, il faut quand même savoir écrire dans le fichier de l'humain sans le détruire. Le coût de l'alternative précédente revient intact, avec en plus deux chemins de code à tenir |
| **Une table `decision` séparée dans le magasin** | l'entité `Decision` figure au modèle de données du cahier des charges, ce qui rendait l'option tentante. Mais une décision a exactement les propriétés du journal : jamais purgée, lue une par une, indétachable de son motif. La séparer la priverait du chaînage, ou en imposerait un second, et deux mécanismes de preuve dérivent |
| **Réutiliser `Outcome::Applied` pour une décision** | laisserait croire qu'une écriture a eu lieu. C'est le motif exact pour lequel `Observed` a été ajouté en Phase 0.5 plutôt que de détourner une variante existante |
| **Mettre la raison et l'échéance dans le champ `diff`, en texte libre** | le champ est couvert par l'empreinte, donc la propriété de preuve tiendrait. Mais la relecture exigerait de reparser une chaîne composite, et une décision se lit trop souvent pour dépendre d'un découpage par convention |
| **Un cinquième `Verdict`, « toléré »** | effacerait le fait derrière la politique : l'item ne serait plus publié en écart, donc l'écart disparaîtrait de la comparaison au lieu d'être qualifié. C'est la correction de l'ADR-0008 prise à l'envers |
| **Une échéance plafonnée (deux ans, par exemple)** | une valeur en dur qu'aucun second cas d'usage ne réclame, donc de la configuration avant le besoin. Afficher la durée à côté de la date rend l'exception permanente visible sans inventer une politique, et D2-06 vise le silence, pas la longueur |
| **Refuser le document sur une acceptation devenue sans objet** | bloquerait l'utilisateur sur un fichier qui décrit correctement son intention, pour une ligne qui n'empêche aucune comparaison. Le refus est réservé à ce qui rend une tolérance indéterminée |

## Conséquences

### Ce que ça nous donne

D2-06 cesse d'être une déclaration sans effet. Une tolérance déclarée change ce
que `ks diff` affiche, son échéance la révoque toute seule, et l'invariant de
couverture interdit qu'un écart toléré disparaisse ou passe pour conforme. D2-07
obtient une trace chaînée, ancrée, jamais purgée, sur un mécanisme qui existe et
qui a été éprouvé en le franchissant.

La Phase 1 conserve la propriété qui autorise à lancer l'outil sur une machine de
production : Keystone ne modifie aucun fichier dont il n'est pas l'auteur
intégral, ne lance aucun processus, n'écrit rien sur le système. Et la frontière
n'est plus une formule commode démentie par trois exceptions, mais trois cercles
qu'on peut citer en revue.

### Ce que ça nous coûte

Un collage manuel après chaque acceptation. Sur sept jours d'observation, cela
représente quelques occurrences, et l'ADR-0017 a déjà accepté ce coût pour git.

Le risque que l'utilisateur ne colle pas, ou colle mal. Une indentation cassée
fait refuser le document entier, avec un message qui nomme la ligne, ce qui est
désagréable et sans gravité. Une acceptation jamais collée laisse l'écart actif,
donc visible : le mode de défaillance montre plus que prévu, jamais moins.

Une variante de plus à `Outcome`, donc une étiquette stable de plus dans un
format de journal, qui ne se renommera jamais. Et un changement de type sur
`DriftStatus::Accepted::expires`, qui casse la compilation de tous ses appelants,
ce qui est le comportement voulu.

### Ce que ça corrige dans la documentation, et qui part au même commit

**D2-07 devient partiellement fausse et doit être réécrite.** Elle affirme que
« chaque acceptation produit une entrée durable et consultable ». Avec cette
décision, une acceptation écrite à la main dans le fichier n'en produit aucune, et
c'est un cas nominal, pas un contournement. Correction à porter au cahier des
charges : la trace couvre les décisions **prises par Keystone** ; celles écrites
directement dans le fichier sont tracées par git, donc par D2-01 et par la
discipline de son propriétaire, ce que l'ADR-0017 a déjà consigné comme une
limite.

**L'ADR-0017 est corrigée sur un point.** Sa décision écrit que « `ks import` et
`ks accept` écrivent le fichier, puis affichent la commande ». C'est vrai pour
`ks import`, faux pour `ks accept` à partir d'ici. Sa décision centrale, ne
jamais exécuter git, n'est pas touchée, et son point n° 3, faire de la trace
D2-07 une entrée du journal de Keystone, est confirmé mot pour mot.

**La surface de référence de la CLI (§10.1) ne porte aucune commande
d'acceptation.** Quinze commandes y sont listées, `ks accept` n'en fait pas
partie, si bien que D2-06 n'a jamais eu d'interface dans le document qui fait
autorité sur les interfaces. La commande décidée ici s'y ajoute, avec `--reason`
et `--expires` obligatoires, ce qui fait tenir D2-06 par la ligne de commande
elle-même : une acceptation sans motif ni échéance ne peut pas être formulée.
`ks journal` y gagne son filtre `--verb`, sans lequel les décisions se
chercheraient à l'œil parmi les scans.

**La feuille de route** coche ses deux lignes de Phase 1 en nommant cette ADR.

**`ks import --force` doit dire ce qu'il emporte.** Le drapeau reste, mais la
commande énumère avant d'écrire ce que le fichier existant porte et qu'elle ne
sait pas reproduire : les acceptations, `inherits`, `owner`, `description`. Une
perte annoncée reste une perte ; une perte silencieuse est un défaut. Ce n'est
pas le sujet du lot, c'est une conséquence directe de son inventaire, et le taire
reviendrait à le découvrir une seconde fois.

**Le squelette de `workstation.yaml` du cahier des charges (§9.1) reste dans
l'ancienne forme imbriquée** que l'ADR-0010 a remplacée, et son exemple
d'`acceptedDrift` porte un chemin, `services.Fax.startupType`, qui n'existe dans
aucun collecteur : le chemin réel est `security.services.fax.startup`. Ce lot ne
répare pas ce paragraphe, mais il le nomme, faute de quoi le prochain lecteur
recopiera un chemin mort.

### Ce que ça ferme

Rien d'irréversible. L'insertion chirurgicale reste écrivable en Phase 2, quand
le moteur d'instantanés donnera un retour arrière que Keystone sait fabriquer
lui-même ; c'est le bon moment pour elle, et ce n'est pas maintenant. Le format
du fichier ne change pas d'un caractère : ce lot lit ce que l'ADR-0016 a déjà
défini, et n'ajoute aucune clé.

### Ce que ça ne garantit pas

**Rien n'empêche l'utilisateur de tenir son fichier sans jamais lancer
`ks accept`.** Les tolérances existeront alors sans aucune trace de décision, et
D2-07 ne sera tenue que par l'historique git, s'il existe. Keystone peut le
constater ; il ne peut pas y remédier sans écrire dans un fichier dont il n'est
pas l'auteur.

**Le journal ne protège pas d'un attaquant privilégié.** L'ancrage et
l'algorithme sont publics : qui obtient SYSTEM réécrit la chaîne entière et
`verify_chain` répond « intacte ». La portée exacte est écrite dans l'ADR-0004,
et il faut l'avoir lue avant de citer ce mécanisme comme une protection.

**Le fichier reste inscriptible par l'adversaire A1.** Une acceptation ajoutée
par un tiers dans `workstation.yaml` fait taire un écart, et rien de local ne le
détecte : Keystone lit ce qu'il trouve. Le journal aide, puisque cette
acceptation n'y aura pas de décision correspondante, mais il n'est pas consulté
par `ks diff`, et la réconciliation a été écartée plus haut pour une raison de
bruit qui reste valable. La parade est une signature ou une ancre externe, ce qui
n'existe pas avant la Phase 3 (SEC-04).

**Ce document ne dit rien de l'épinglage ni des exclusions**, que D2-07 nomme à
côté de l'acceptation. Ils suivront la même répartition s'ils s'y prêtent, et
mériteront leur propre décision s'ils ne s'y prêtent pas. Rien ici ne prétend
l'avoir tranché pour eux.

---

## Amendement du 2026-08-17 — la décision n° 2 est renversée

> Ce qui suit a été écrit **après** la mise en œuvre, et l'annonce. Le corps du
> document reste tel qu'il était : une ADR qu'on récrit pour qu'elle tombe juste
> ne vaut plus rien, et l'intérêt de celle-ci tient précisément à ce qu'elle a
> pesé une option qu'elle a écartée, avec ses raisons, avant qu'on l'essaie.

**Ce qui change.** `ks accept` **écrit** le fichier d'état désiré, sur `--apply`,
par l'insertion chirurgicale que l'alternative n° 5 décrivait et écartait. Sans
`--apply`, il n'écrit rien du tout — ni le fichier, ni le journal.

### Le coût que l'ADR redoutait était réel, et il se paie autrement qu'en soin

L'objection portait juste : « un éditeur positionnel qui se trompe écrit un
fichier corrompu, et ce fichier est la source de vérité du poste ». Promettre
d'être prudent n'y répond pas. Deux mécanismes y répondent, et aucun des deux
n'est une intention :

1. **La préservation se prouve par soustraction.** Le test
   `linsertion_ne_touche_que_le_bloc_accepted_drift` retire du résultat
   exactement les lignes ajoutées et compare les octets restants à l'original,
   commentaires, indentation et lignes vides compris. Ce n'est pas une relecture
   attentive : c'est une égalité.
2. **Le document obtenu est relu par le lecteur du produit avant d'atteindre le
   disque.** `EtatDesire::lire` s'exécute sur la chaîne résultante, et la
   commande refuse d'écrire si elle échoue, ou si la tolérance demandée ne s'y
   retrouve pas. Un fichier que Keystone n'accepterait plus n'est donc pas
   atteignable par ce chemin, quelle que soit la faute de l'éditeur positionnel.

La sauvegarde déposée à côté, à laquelle l'ADR objectait à raison, **n'existe
pas** : elle n'est pas nécessaire. Le fichier est versionné en git, l'écriture
est atomique, et la commande propose la ligne `git add` comme `ks import` le fait
déjà (ADR-0017).

### Deux faits ont emporté la décision

**Le premier est une incohérence de la position d'origine.** `ks import --force`
réécrivait ce même fichier **en entier**, donc détruisait déjà tolérances,
`inherits`, `owner` et `description`. Refuser d'y insérer cinq lignes tout en
acceptant de l'écraser n'était pas tenable. Ce défaut-là est corrigé au même
commit, et dans l'autre sens : l'import **conserve** désormais ce qu'aucun
collecteur ne sait produire.

**Le second corrige l'ADR sur P2.** Telle qu'elle était rédigée, la commande
consignait la décision au journal **par défaut**, sans drapeau. Or `ks scan` ne
journalise que sur `--record`, et son code dit pourquoi : « un scan qui
journaliserait sans qu'on l'ait demandé contredirait sa propre bannière, et le
contredirait en silence ». Une décision consignée alors que le fichier n'a pas
bougé décrit une tolérance qui n'existe nulle part ; c'est écrire par omission, ce
que P2 interdit. Sans `--apply`, la commande affiche donc le bloc et s'arrête, et
c'est cet affichage qui est le diff exigé par P2.

### Ce qui n'a pas changé

La décision n° 1 tient intégralement : le fichier porte les tolérances en
vigueur et fait foi pour le verdict, le journal porte la décision et son motif.
Les décisions n° 3 à n° 6 tiennent telles quelles, et l'alternative « un
cinquième `Verdict`, toléré » reste écartée pour la raison qui y est écrite — un
écart toléré est publié en écart, annoté de son échéance.

### Ce que ça ne garantit toujours pas

L'insertion ne sait pas où l'humain **aurait voulu** que la ligne aille : elle la
pose en queue de la liste, ce qui est chronologique et arbitraire à la fois.
Elle ne réordonne rien, ne trie rien, et ne commente rien. Un fichier tenu à la
main avec des sections thématiques verra donc ses tolérances s'accumuler au même
endroit, ce qui est un choix par défaut et non une décision de conception.
