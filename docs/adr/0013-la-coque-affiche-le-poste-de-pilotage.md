# ADR-0013 — La coque affiche le poste de pilotage, pas un document

- **Statut** : Accepté
- **Date** : 2026-08-03
- **Remplace** : la décision n° 1 de [ADR-0012](0012-coque-de-bureau-tauri.md). Les
  décisions n° 2 à 4 de cette ADR (module de bibliothèque plutôt que copie, coque
  non privilégiée, aucune ressource réseau) restent en vigueur telles quelles.
- **Exigences concernées** : P2, P4, P6, D2-02, D2-04, moment de vérité M1

## Contexte

L'ADR-0012 a tranché, la veille : la coque afficherait le HTML de
`rapport::construire`, et jamais la maquette. Le raisonnement était bon, et sa
conclusion trop étroite.

Bon, parce que la maquette `design/keystone-cockpit.html` est peuplée de valeurs
inventées — `WKS-ORION-04`, 312 items, une posture à 94 — et qu'un exécutable
qui les afficherait sous une icône « Keystone » serait un produit qui ment.

Trop étroite, parce qu'elle a confondu **la maquette** et **le poste de
pilotage**. Le cockpit est l'interface du produit ; le rapport n'en est qu'un
document. Livrer le second en refusant le premier, c'était répondre à un
problème de véracité par une amputation de périmètre — alors que la bonne
réponse tenait dans une troisième voie : **brancher le poste de pilotage sur les
valeurs réelles**, et faire dire à l'écran, explicitement, tout ce qui n'est pas
encore mesurable.

L'utilisateur a dû le demander deux fois. C'est le signe que la prudence était
passée devant la demande, ce qui n'est pas de la rigueur.

## Options

| Option | Ce qu'elle coûte, ce qu'elle rapporte |
|---|---|
| **S'en tenir à l'ADR-0012** | Rien à faire, et une interface qui reste un tableau. Le produit conçu n'existe nulle part hors d'un fichier de maquette. |
| **Empaqueter la maquette telle quelle** | Immédiat, et disqualifiant : une fenêtre intitulée Keystone affichant 312 items d'une machine qui n'existe pas. Refusé pour la même raison qu'hier. |
| **Brancher le poste de pilotage sur les items réels** | Retenue. Ce qui est mesuré s'affiche ; ce qui ne l'est pas se déclare, avec sa raison et l'action qui le rendrait disponible. |

## Décision

**La coque affiche le poste de pilotage, alimenté par `Inventory::collect_all()`.
Le rapport devient l'un de ses écrans.**

La règle qui gouverne l'écran, et qui n'est pas négociable :

> **Aucun nombre affiché n'est inventé.** Chaque valeur vient d'un item relevé,
> ou l'écran dit qu'elle n'est pas collectée — avec la raison. Jamais de valeur
> par défaut, jamais de zéro qui ressemble à une mesure.

Trois éléments du cockpit tombent sous cette règle aujourd'hui, et la façon dont
ils la respectent est le cœur de cette décision.

**L'anneau de posture n'affiche pas de chiffre.** Une posture composite se
calcule contre une référence, et aucun `workstation.yaml` n'est chargé. Le
principe P6 est explicite : un indicateur composite qu'on ne peut pas déplier en
ses composantes exactes est une décoration. L'anneau ne trace donc aucun arc,
annonce « pas encore calculable », et donne la raison en clair. C'est exactement
le moment M1 du brief : la machine a été lue, rien n'a été modifié, et l'action
proposée est d'adopter cet état comme référence.

**La dérive n'affiche pas « 0 écart ».** Sans état désiré, il n'y a pas zéro
écart : il n'y a pas de comparaison. Publier zéro serait précisément le défaut
que `Verdict::Incomparable` a été créé pour empêcher, réintroduit par l'écran
après avoir été corrigé dans le noyau. La vue publie les quatre verdicts, dont
115 « non contraint », et écrit « sans objet » là où le calcul n'a pas de sens.

**Les mises à jour se déclarent non collectées.** Le domaine D3 n'a pas de
collecteur ; l'écran le dit, avec la référence d'exigence.

### Ce qui rend la règle tenable

Une règle de rédaction se contourne ; une forme de type, non. Trois mécanismes
la portent dans le code plutôt que dans une consigne :

- les valeurs vivent dans **une seule table indexée par chemin d'item** ; les
  écrans ne transportent que des listes de chemins, jamais de nombres ;
- le type qui décrit un élément non calculable **n'a aucun champ numérique** :
  il est structurellement incapable de porter un chiffre ;
- le comptage des verdicts est un `match` **exhaustif sans bras `_`** — ajouter
  une variante à `Verdict` casse la compilation de la coque, comme elle casse
  déjà celle du noyau.

## Conséquences

### Ce que ça nous donne

Le produit conçu existe, sur des données vraies. Et l'écran le plus difficile du
brief — celui qui dit « je n'ai pas encore de quoi calculer » sans avoir l'air
d'une erreur — est écrit, donc éprouvé, alors qu'il aurait été le dernier à
l'être.

### Ce que ça nous coûte

Une seconde implémentation du CSS du cockpit, à côté de la maquette. La
duplication n'est pas supprimée — un fichier de maquette qu'on peut ouvrir seul a
de la valeur — elle est **vérifiée** : une étape de CI compare les jetons des
trois copies à `design/tokens.css`. Elle a d'ailleurs immédiatement trouvé une
divergence dans le rapport, qui traînait sans que personne la voie.

### Ce que ça ne garantit pas

**Les écrans ne sont pas terminés.** L'attribution de l'espace par consommateur,
l'inventaire Hyper-V et la timeline forensique restent à écrire, et l'écran le
dit à chaque fois plutôt que de le taire. La Phase 5 reste la Phase 5 : ce qui
est livré ici, c'est le socle et la règle qui le gouverne.

**Et la règle ne se prouve pas d'elle-même.** Elle tient tant que les trois
mécanismes ci-dessus tiennent. Le jour où un écran calculera une valeur au lieu
de la lire, aucun test existant ne le verra : il faudra en écrire un.
