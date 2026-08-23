# ADR-0022 — Un taux d'occupation n'est pas un relevé, c'est un rendu

- **Statut** : Accepté le 2026-08-23.
- **Date** : 2026-08-23
- **Exigences concernées** : D4, D2-03, P6, NF-05
- **Précise** : ADR-0009 (ce qui a vocation à être déclaré) et ADR-0015 (le jeton
  vit dans l'item, la phrase française vit dans l'affichage).

## Contexte

Le collecteur d'espace calculait la taille totale du volume **et** les octets
occupés, puis jetait les deux pour ne publier qu'un seul item :
`space.volume[X].used_percent`.

L'écran Espace affichait donc « 65 % » et pas un octet. Or la règle de voix du
projet dit, mot pour mot : « **Le chiffre avant l'adjectif. « 47 Go
récupérables », pas « beaucoup d'espace ».** » Un ratio sans dénominateur ne se
déplie pas : on ne peut pas savoir si 65 % vaut trente gigaoctets ou six cents,
ce qui contrevient à P6.

Le manque avait une seconde conséquence, plus lourde. Le collecteur de
virtualisation lit le `BasePath` du registre pour aller mesurer chaque
`ext4.vhdx`, et ne publiait que la taille. Mesuré sur la machine de référence :
trois distributions WSL pèsent **96,6 Gio** sur un volume dont **626,8 Gio** sont
occupés, soit un sixième de ce qui le remplit. Keystone mesurait les deux faits
dans le **même scan** et ne pouvait pas les rapprocher : d'un ratio, rien ne se
retranche.

L'écran qu'on ouvre pour demander « pourquoi mon disque est plein » répondait
donc qu'il n'en savait rien, alors que la feuille de route désigne ce fichier
comme « le premier poste d'occupation d'un poste de développement, et personne ne
le sait ».

## Décision

**1. Le collecteur publie des octets, jamais un pourcentage.**

`space.volume[X].total_bytes` en `Nature::Constat` — la taille d'un volume ne
dérive pas d'elle-même, elle change quand on repartitionne. `space.volume[X].used_bytes`
en `Nature::Mesure` — elle bouge à chaque scan.

**2. `used_percent` est retiré, et non conservé à côté.**

Le publier en plus donnerait **deux sources pour un même fait**, et ce dépôt
écrit partout que deux sources pour un même fait divergent. Le taux se calcule à
l'affichage, exactement comme un jeton s'y traduit en phrase française (ADR-0015)
et comme un nombre d'octets s'y écrit en gibioctets par `lisible::octets`.

**3. Le collecteur de virtualisation publie le volume de chaque disque.**

`virtualization.wsl[x].volume`, en `Nature::Constat`, la **lettre seule**. Jamais
le chemin complet : un `BasePath` porte le nom de l'utilisateur et celui du
paquet, c'est-à-dire plus de surface que la question n'en demande. La lettre est
**dérivée** du chemin déjà lu ; quand elle ne l'est pas — chemin UNC, forme
inattendue —, l'item vaut `Absent`, jamais une lettre devinée.

**4. L'attribution est partielle, et se dit partielle.**

L'écran nomme ce qu'il attribue, avec le chemin de l'item d'où vient chaque part,
et nomme le reste « non attribué » plutôt que de le fondre dans le total. La
somme des parts et du non attribué fait exactement l'occupation, jamais plus :
c'est un invariant, pas une présentation.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Garder `used_percent` **et** ajouter les deux items d'octets | trois items pour deux faits, dont un dérivable des autres. Le jour où le collecteur change une borne, le taux publié et le taux calculé divergent, et rien ne dit lequel croire |
| Publier le `BasePath` complet plutôt que la lettre | expose le nom de l'utilisateur et celui du paquet pour répondre à une question qui n'en a pas besoin. La surface exposée se justifie par l'usage, pas par la commodité |
| Deviner le volume quand le chemin n'est pas analysable | une attribution inventée est pire qu'une attribution manquante : elle se croit. C'est le symétrique exact de l'incident où marquer les relevés `Keystone` rendait `Unknown` inatteignable |
| Attribuer aussi les caches d'outils, les instantanés, les données de travail | ce sont les consommateurs que le domaine D4 apportera, avec la quarantaine qui rend leur récupération réversible. Les annoncer ici sans les mesurer serait cocher une case sur du vide |
| Afficher le taux et taire les octets | c'est l'état d'avant, et la règle de voix du projet le condamne nommément |

## Conséquences

**Ce que ça donne.** L'écran Espace dit « 626,8 Gio occupés sur 952,8 Gio »,
puis « 96,6 Gio attribués » avec le détail par distribution et le chemin de
chaque item, puis « 530,2 Gio non attribué ». Le chiffre passe avant le ratio, et
le compte se referme.

**Ce que ça coûte.** Un item de plus par volume, et un item de plus par
distribution. Le taux devient un calcul d'affichage, donc un endroit de plus où
une division peut se tromper ; c'est la contrepartie assumée d'une source unique
par fait.

**Ce que ça corrige dans la documentation.** `docs/03-ARCHITECTURE.md` citait
`space.volume[…].used_percent` comme item d'exemple du domaine Espace : la ligne
est corrigée dans le même commit.

**Ce que ça ne corrige pas.** L'ADR-0009 emploie `used_percent` comme exemple de
`Nature::Mesure`, à trois reprises. Une ADR est un relevé daté : elle se remplace,
elle ne se réécrit pas. L'exemple y reste donc, et ce document-ci est le successeur
qui dit pourquoi il est périmé.

**Ce que ça ne garantit pas.** La part non attribuée reste sans nom. Keystone ne
mesure ni les caches de chaînes d'outils, ni les instantanés, ni les données de
travail, et n'avance **aucune quantité récupérable**. Une attribution partielle
présentée comme complète serait le défaut que ce dépôt traque ; l'écran l'énonce
donc en toutes lettres, au lieu de laisser le lecteur le déduire.
