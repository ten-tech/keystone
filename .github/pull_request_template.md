<!--
  Le sujet de la PR suit la même convention que les commits :
      <type>(<portée>): <sujet> (<EXIGENCE>)
-->

## Ce que ça change

<!-- Une à trois phrases. Le POURQUOI ; le diff dit déjà le quoi. -->

**Exigences** : <!-- D5-03, SEC-02, NF-07… ou « aucune » -->

## Portes du projet

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` passent
- [ ] La documentation part **dans le même commit** que le code qu'elle décrit
- [ ] Aucune exigence rendue fausse par ce changement — ou alors elle est corrigée ici

## Une ADR est-elle nécessaire ?

Obligatoire pour : **tout nouveau verbe du broker** · tout changement de la surface d'API ·
**toute nouvelle dépendance de `ks-broker`** · tout choix de technologie · tout renversement
d'un principe P1 à P10.

- [ ] Non applicable
- [ ] Oui → `docs/adr/NNNN-<sujet>.md` incluse dans cette PR

## Si `ks-broker` est touché

Lecture de [`docs/04-MODELE-DE-MENACE.md`](../docs/04-MODELE-DE-MENACE.md) **obligatoire**.

Question éliminatoire : *ce verbe permet-il, directement ou par détour, d'exécuter du code
arbitraire ?* Si la réponse n'est pas un non argumenté, le verbe ne passe pas.

- [ ] Il sait **se simuler** (P2)
- [ ] Il sait **s'annuler** (P3)
- [ ] La **présence humaine** est exigée si son coût est réel (SEC-08)
- [ ] Il est **idempotent** — ou l'écart est justifié
- [ ] La barrière SEC-02 s'exécute toujours (`1 passed`, pas `0 passed; N filtered out`)

## Si une couleur ou un token change

- [ ] Contrastes **recalculés** contre les trois surfaces, pas ajustés à l'œil
- [ ] [`design/palette-validation.md`](../design/palette-validation.md) mise à jour **dans ce commit**
- [ ] Aucune ressource réseau ajoutée (principe P5, « Local, point final »)

## Écriture système

- [ ] Ce changement **n'écrit rien** sur la machine — phases 0 et 1
- [ ] Il écrit, et a donc été éprouvé **dans la VM de labo**, jamais sur l'hôte
