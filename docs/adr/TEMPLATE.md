# ADR-XXXX — Titre à l'infinitif ou au participe

- **Statut** : Proposé | Accepté | Remplacé par ADR-YYYY
- **Date** : AAAA-MM-JJ
- **Exigences concernées** : Dxx-yy, SEC-zz, Pn

## Contexte

Le problème à résoudre, et les contraintes réelles. Pas la solution.

## Décision

Ce qu'on fait, en une ou deux phrases affirmatives.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| | |

C'est **la section la plus utile** de l'ADR. Dans six mois, la question ne sera pas
« qu'a-t-on décidé » — le code le dit — mais « pourquoi n'a-t-on pas fait l'autre
chose ». Une ADR sans alternatives écartées ne sert à rien.

## Conséquences

### Ce que ça nous donne

### Ce que ça nous coûte

Sois honnête ici. Une décision sans coût est une décision mal analysée.

### Ce que ça ferme

Les portes qu'on se condamne à ne plus prendre facilement.

---

## Section obligatoire pour tout nouveau verbe du broker

Les quatre questions du modèle de menace (§7). Aucune réponse ne peut être omise.

1. **Sait-il se simuler ?** Sinon il ne peut pas être appliqué (P2).
2. **Sait-il s'annuler ?** Sinon il ne peut pas être automatisé (P3).
3. **Exige-t-il une présence humaine ?** Si son coût est réel, oui (SEC-08).
4. **Est-il idempotent ?** Sinon, pourquoi, et comment le rendre sûr malgré tout.

Et la question qui précède les quatre autres : **ce verbe permet-il, directement ou
par détour, d'exécuter du code arbitraire ?** Si la réponse n'est pas un « non »
argumenté, le verbe ne passe pas.
