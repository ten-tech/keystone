# ADR-0010 — La forme de `workstation.yaml`, et qui est source de vérité de son schéma

- **Statut** : Proposé
- **Date** : 2026-08-02
- **Exigences concernées** : D2-01, D2-02, D2-10, P6

## Contexte

`schema/workstation.schema.json` existe depuis le premier jour, 380 lignes, et
la CI le valide — le travail `schema` de `ci.yml` charge
`schema/examples/workstation.yaml` et le vérifie contre lui. Le schéma est donc
cohérent **avec son exemple**. Avec rien d'autre.

### Le schéma et le modèle sont deux formes sans rapport

Le schéma décrit un document imbriqué, calqué sur les domaines du cahier des
charges :

```yaml
platform: { secureBoot: enabled, vbs: { enabled: true, hvci: enforced } }
updates:  { windows: { quality: auto }, rings: { canary: [...] } }
```

Le modèle Rust décrit une liste plate d'items à chemin pointé :

```
security.platform.secure_boot          activé
security.platform.hvci_policy          activé
security.services.windefend.startup    automatique
```

Aucun code ne lit le schéma. Aucune correspondance n'existe entre les deux. Et
**ni l'un ni l'autre n'est un sur-ensemble** :

- le schéma exprime `backup`, `profiles`, `updates.rings`, `wsl.distros[].resources`,
  pour lesquels **aucun collecteur ne produit d'item** ;
- le modèle produit `security.services.*.startup` (6 items), `security.clock.*`,
  `security.firewall.*.enabled`, `virtualization.wsl[*].interop`, que **le schéma
  ne sait pas exprimer**.

Générer le yaml de M1 dans la forme actuelle du schéma exigerait donc une table
de correspondance chemin ↔ champ, écrite et maintenue à la main. Ce serait une
**troisième** source de vérité, à côté des collecteurs et du schéma — ce que la
règle du projet interdit, et pour la raison exacte qu'on observe ici : les deux
premières ont déjà divergé sans que rien ne le signale.

### Un piège de sérialisation mesuré — **corrigé depuis**

`ItemValue` est `#[serde(untagged)]`, et `Absent` en **était** une variante
unité. Mesuré le 2026-08-02, avant correction :

```
Some(ItemValue::Absent)  →  {"desired":null}
None                     →  {"desired":null}
aller-retour de Some(Absent)  →  None
```

Autrement dit, « **cet item ne doit pas exister** » et « je ne contrains pas cet
item » étaient indiscernables après un aller-retour. Ce sont deux intentions
opposées. Le commentaire de `Illisible` avait déjà identifié la famille du piège
et choisi une variante de forme objet pour l'éviter ; `Absent` y était resté
exposé.

**Le même jour, `Absent` a reçu une forme objet sur le fil** — `{"absent":true}`,
champ obligatoire et `deny_unknown_fields`, la variante restant unité côté Rust
pour ne toucher aucun site d'appel. Le piège d'ordre d'`untagged` est désamorcé
par le contenu et non par la déclaration : `Absent` exige la clé `absent`,
`Illisible` exige `raison`, aucun ordre ne peut plus décider. Un test d'aller-
retour couvre toutes les variantes, gardé par un `match` exhaustif sans bras
`_` — ajouter une variante à `ItemValue` casse la compilation avant le test.

Cette section reste ici, au passé, parce que le raisonnement qui suit s'appuie
dessus : **la décision n° 3 ne change pas**. Son motif principal tient toujours
sans ce piège — `Illisible` n'a aucun sens dans un désir, et un type de désir
qui pourrait l'exprimer serait un type mal fait.

## Décision

**1. Le yaml de la Phase 1 porte une section `desired:` indexée par chemin d'item.**

```yaml
apiVersion: keystone/v1
kind: Workstation
metadata:
  name: PC-DU-DUC

desired:
  # security — posture de la plateforme
  security.platform.hvci_policy: true
  security.platform.lsa_protection: "activée, sans verrou UEFI"
  security.defender.realtime: true
  security.services.windefend.startup: automatique
  security.firewall.public.enabled: true

acceptedDrift: []
```

L'espace des chemins d'items produit par `ks-collectors` est **la** source de
vérité. Le yaml le référence, le schéma en dérive.

**2. Le schéma JSON est généré, plus jamais écrit à la main.**

Génération par `schemars` depuis les types Rust de `ks-cli`. Un travail de CI
régénère le schéma et **échoue si le fichier commité en diffère** : la
divergence devient impossible plutôt qu'improbable.

**3. Le côté désiré ne réutilise pas `ItemValue`.**

Un type distinct, `Desire`, avec sa propre représentation sérialisée, conçue
pour un fichier écrit à la main :

```yaml
security.services.fax.startup: désactivé      # une valeur
security.defender.exclusions.paths: !absent   # doit ne pas exister
```

Le désir et le constat ne portent pas les mêmes possibilités : un constat peut
être `Illisible`, un désir jamais. Rendre les états illégaux inconstructibles
vaut mieux que les refuser à l'exécution.

**4. L'ancienne forme imbriquée sort du chemin de validation.**
`schema/examples/workstation.yaml` et `schema/workstation.schema.json` sont
réécrits **dans le même commit** que la génération. Une documentation qui décrit
un produit disparu est pire qu'une absence de documentation.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Garder la forme imbriquée et écrire la table de correspondance | troisième source de vérité, coût O(n) permanent, dérive silencieuse à chaque item ajouté. C'est le mode de défaillance que ce projet corrige depuis la Phase 0 |
| Garder la forme imbriquée et générer le schéma depuis des types Rust miroirs | déplace la table dans le code sans la supprimer : les types miroirs doivent toujours suivre les collecteurs à la main |
| Garder le schéma écrit à la main | l'état actuel : il valide un exemple fictif et ne dit rien du produit |
| La forme imbriquée comme sucre au-dessus de l'espace des chemins | c'est le bon point d'arrivée **si** le fichier plat se révèle illisible à l'usage. Il reste atteignable : la forme plate ne ferme pas cette porte. On ne paie pas ce coût avant d'avoir constaté le besoin |
| Réutiliser `ItemValue` côté désiré | mesuré indiscernable de « non déclaré » pour `Absent`, et laisse un désir porter `Illisible`, ce qui n'a aucun sens |

## Conséquences

### Ce que ça nous donne

Zéro dérive entre le schéma et le modèle, par construction et non par vigilance.
`ks import` écrit ce qu'il vient d'observer, sans traduction. `ks diff` compare
des chemins à des chemins. Et une réponse nette à « qui est la source de vérité » :
les collecteurs.

### Ce que ça nous coûte

Le regroupement visuel par domaine, que la forme imbriquée offrait gratuitement.
Atténuation : `ks import` écrit les clés triées et insère un commentaire de
section par domaine — ce que montre l'exemple ci-dessus.

Deux dépendances. `schemars`, version stable 1.2.2, MSRV 1.74, vérifiée sur
crates.io le 2026-08-02 : compatible avec notre 1.85. Et un lecteur YAML, pour
lequel **aucun choix n'est confortable**, mesuré le même jour :

| Crate | Version | Fait |
|---|---|---|
| `serde_yaml` | 0.9.34+deprecated | déprécié, dépôt archivé le 2024-03-25 |
| `serde_yaml_ng` | 0.10.0 | fork, dernière publication mai 2024, adossé à `unsafe-libyaml` (C transpilé) |
| `serde_norway` | 0.9.42 | fork, dernière publication déc. 2024, même socle |
| `noyalib` | 0.0.18 | pur Rust, `forbid(unsafe_code)`, mais **MSRV 1.86 — au-dessus de la nôtre** |
| `serde-saphyr` | 1.0.0 | publiée le 2026-07-31, **il y a deux jours**, pas de `rust-version` déclarée |

`noyalib` est la **troisième** occurrence du piège déjà rencontré avec
`libsqlite3-sys` et `wmi` : une MSRV de dépendance supérieure à la nôtre, que ni
le manifeste ni le résolveur ne signalent, et qui n'échoue qu'à la compilation.
Le choix se tranche **en compilant à 1.85**, pas en lisant, et se documente dans
`Cargo.toml` comme les deux précédents.

### Ce que ça ferme

Rien d'irréversible. La forme imbriquée reste constructible plus tard comme
présentation, au-dessus du même espace de chemins.

### La dette prise, et sa condition de remboursement

Il n'existe pas de **catalogue statique** des chemins valides : le seul
référentiel est ce qu'un scan a observé. Conséquence assumée : une faute de
frappe dans un chemin écrit à la main est indiscernable d'un item légitimement
disparu (une distribution WSL supprimée, par exemple). `ks diff` les affiche
tous deux comme « déclaré, non observé », en nommant les deux causes possibles.

Ce n'est pas un oubli mais une application de « pas d'abstraction avant la
deuxième utilisation » : le catalogue aura son second usage en Phase 2, quand le
broker devra savoir quel verbe écrit quel chemin. Il se construira là, avec les
deux besoins sous les yeux.
