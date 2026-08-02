# 08 — Conventions

## Les deux dépôts

Distinction importante, et source de confusion si on ne la pose pas tout de suite :

| Dépôt | Contenu | Visibilité |
|---|---|---|
| **celui-ci** | le code de Keystone, la documentation, le schéma, un exemple de configuration | public, à terme |
| **le dépôt de ton poste** | *ton* `workstation.yaml` réel, *ta* `base.yaml`, tes surcouches | privé, séparé, chez toi |

`.gitignore` bloque déjà `/workstation.yaml` et `/base.yaml` à la racine :
la configuration réelle d'une machine n'a rien à faire dans le dépôt de l'outil. Elle
contient les noms de tes machines, tes chemins, tes exceptions, la topologie de ton
réseau. C'est de la reconnaissance offerte à qui la lirait.

L'exemple versionné est `schema/examples/workstation.yaml`, et il est fictif.

## Git

### Branches

```
main                    protégée · toujours verte · ce qu'un tiers clone
  ↑ fusion par PR, après revue
dev                     intégration · cible par défaut des PR
  ↑ fusion par PR
feat/<domaine>-<sujet>  ex. feat/d5-magasin-certificats
fix/<sujet>
docs/<sujet>
adr/<numero>-<sujet>    une ADR seule, pour qu'elle soit discutée sans code
```

**Pourquoi deux branches longues et pas une.** Le critère de sortie de la Phase 6
est qu'un tiers installe Keystone à partir de la seule documentation : `main` est
donc ce qu'il clone, et elle ne doit jamais être un état de passage. `dev` reçoit
l'intégration ; `main` ne reçoit que ce qui est fini.

Les branches courtes partent de `dev` et y retournent. Elles sont **éphémères** :
une branche qui vit plus d'une semaine est un lot mal découpé.

**Ce que `main` refuse, et c'est appliqué par la protection de branche :** le push
direct, la fusion sans revue, la fusion sans que les portes de qualité passent, et
la réécriture d'historique. Les zones sensibles — `crates/ks-broker/`, `docs/adr/`,
le modèle de menace, la CI, la palette — exigent en plus la revue de leur
propriétaire ([`../.github/CODEOWNERS`](../.github/CODEOWNERS)).

### Hooks

Les conventions ci-dessous ne valent que si quelque chose les applique. Trois hooks
versionnés dans [`../.githooks/`](../.githooks/), à activer une fois par clone :

```powershell
.\scripts\dev-hooks.ps1
```

| Hook | Ce qu'il vérifie | Coût |
|---|---|---|
| `commit-msg` | Conventional Commits, portée connue, sujet ≤ 72 car., référence d'exigence | instantané |
| `pre-commit` | format, secrets évidents, fichiers interdits, marqueurs de conflit | ~1 s |
| `pre-push` | `fmt` + `clippy -D warnings` + `test` + barrière SEC-02 rejouée | 2 à 4 min |

`--no-verify` existe et se justifie en revue. Un contournement silencieux, non.

### Messages de commit

Convention *Conventional Commits*, avec **la référence d'exigence dans le sujet**.
C'est ce qui rend l'historique traçable jusqu'au cahier des charges :

```
feat(d5): détecte l'ajout d'une CA racine dans le magasin (D5-03)

Compare le magasin Root à l'empreinte de référence à chaque scan. Une
autorité ajoutée depuis le dernier passage produit un Finding, et si sa
provenance est indéterminable, une alerte immédiate.

Ne couvre pas encore le magasin par utilisateur — suite en 0.2.
```

Portées : `core`, `cli`, `broker`, `collectors`, `agent`, `d1` à `d16`, `sec`, `docs`,
`ci`, `design`, `schema`.

### La règle qui compte

> **Le code et sa documentation partent dans le même commit.**

Si un changement rend une exigence fausse, corrige l'exigence dans le même commit. Une
documentation qui décrit un produit disparu est pire qu'une absence de documentation :
elle est crue.

## Code Rust

### Non négociable

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Les trois tournent en CI. `main` ne les enfreint jamais.

### `unsafe`

| Crate | Politique |
|---|---|
| `ks-core` | `forbid` — il est pur et portable, aucun besoin |
| `ks-cli` | `forbid` |
| `ks-agent-linux` | `forbid` |
| `ks-collectors` | `warn` — inévitable côté Win32, chaque bloc justifié par un commentaire |
| `ks-broker` | `warn` — idem, et sous revue systématique |

Tout bloc `unsafe` porte un commentaire `// SAFETY:` qui explique **pourquoi**
l'invariant tient. « C'est comme ça dans la doc Microsoft » n'est pas une
justification.

### Nommage

Le code est en **anglais** (identifiants, types, fonctions) ; les commentaires, la
documentation et les messages destinés à l'utilisateur sont en **français**. Les noms
de tests sont en français aussi, parce qu'un nom de test est une phrase qui décrit un
comportement, et qu'on veut la lire dans la langue du projet :

```rust
#[test]
fn une_acceptation_expiree_redevient_convergeable() { … }
```

### Les tests qui comptent

Trois familles, par ordre de valeur :

1. **Les barrières de conception** — un test qui casse quand quelqu'un enfreint un
   principe. Exemple : `aucun_verbe_ne_transporte_dexecution_arbitraire`. Ces tests ne
   vérifient pas un comportement, ils **provoquent une conversation** avant la fusion.
2. **Les invariants** — P2, P3, D2-06, D12-03. Ils encodent les règles du produit.
3. Les tests de comportement classiques.

Et une exigence explicite (NF-07) : **un chemin de retour arrière non testé est réputé
inexistant.** Chaque rollback est éprouvé par injection de fautes, pas par relecture.

## Messages destinés à l'utilisateur

Ils suivent les cinq règles de voix du brief de design (§1.3). Rappel :

1. Ce qui s'est passé → ce que ça implique → ce qu'on peut faire. Dans cet ordre.
2. Jamais de code d'erreur nu ; le code technique vit dans un champ `detail`.
3. Jamais de reproche. Pas « vous avez oublié », mais « disponible depuis 12 jours ».
4. Jamais de superlatif d'urgence, jamais de majuscules criées, jamais de `!`.
5. Le chiffre avant l'adjectif. « 47 Go récupérables », pas « beaucoup d'espace ».

Et zéro émoji dans l'interface produit — les états passent par icône **plus** libellé.

C'est pour cela que `ks_core::Error` sépare `message` (une phrase) et `detail` (le
code technique) : la structure de l'erreur impose la règle.

## ADR — décisions d'architecture

Une décision structurante = un fichier dans [`adr/`](adr/), numéroté, **jamais
réécrit**. Si une décision est renversée, on écrit une nouvelle ADR qui remplace
l'ancienne, et l'ancienne passe en `Superseded`. L'historique des raisonnements a plus
de valeur que la propreté du répertoire.

Une ADR est obligatoire pour :

- **tout nouveau verbe du broker** — avec les quatre questions du modèle de menace ;
- tout changement de la surface de l'API ;
- toute nouvelle dépendance de `ks-broker` ;
- tout choix de technologie ;
- tout renversement d'un principe P1 à P10.

Modèle : [`adr/TEMPLATE.md`](adr/TEMPLATE.md).

## Dépendances

```powershell
cargo audit        # vulnérabilités connues
cargo deny check   # licences, doublons, sources
```

### Ce qui est épinglé, et ce que Dependabot ne couvre pas

Tout ce qui influence un build est épinglé : `Cargo.lock` versionné et `--locked`
partout, actions de CI par SHA complet, images de runner par version nommée,
toolchain Rust par version exacte.

**Rust n'a pas de canal de support long terme** — seulement stable, beta et
nightly. « stable » désigne la dernière version, qui change toutes les six
semaines : l'épingler revient à accepter qu'un build reproductible aujourd'hui ne
le soit plus dans six semaines. D'où la version exacte dans
[`../rust-toolchain.toml`](../rust-toolchain.toml).

Dependabot couvre **les actions et les crates**. Il ne couvre ni les images de
runner, ni la version de Rust. Deux choses se relèvent donc à la main, et se
vérifient **à la source** — jamais de mémoire :

| Quoi | Où | Source à consulter |
|---|---|---|
| Image de runner | `runs-on:` dans le workflow | [`actions/runner-images`](https://github.com/actions/runner-images) |
| Version de Rust | `rust-toolchain.toml` **et** la révision de `dtolnay/rust-toolchain` | [releases de Rust](https://github.com/rust-lang/rust/releases) |

La version de Rust est le **seul** endroit du dépôt où un numéro est écrit deux
fois. C'est délibéré : cette action ne lit pas `rust-toolchain.toml`, et une
révision `@stable` en face d'un fichier épinglé ferait télécharger deux toolchains
par job. Les deux se déplacent ensemble, ou pas du tout.

Une image de runner épinglée **finit par être retirée** : GitHub annonce la
dépréciation, puis programme des coupures. Sans revue, la CI casse un matin sans
qu'aucun commit n'ait bougé.

Sur `ks-broker`, chaque dépendance ajoutée est une surface ajoutée sur un composant
privilégié. Elle passe par une ADR, et la question « peut-on faire sans ? » est posée
sérieusement à chaque fois.

## Design

Les valeurs de `design/tokens.css` sont **vérifiées par calcul** : contraste WCAG contre
les trois surfaces, bande de luminosité, plancher de chroma, séparation en vision
déficiente protan / deutan / tritan. Voir
[`../design/palette-validation.md`](../design/palette-validation.md).

**Ne pas substituer une couleur à l'œil.** Si une teinte doit changer, recalculer, et
mettre à jour la note de validation dans le même commit.
