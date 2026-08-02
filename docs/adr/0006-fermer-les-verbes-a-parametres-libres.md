# ADR-0006 — Fermer les deux verbes à paramètres libres du broker

- **Statut** : Accepté
- **Date** : 2026-08-02
- **Exigences concernées** : SEC-02, SEC-08, P2, P3, P10

## Contexte

Le §7 du modèle de menace nomme les verbes interdits. Le quatrième de la liste
est écrit ainsi, mot pour mot :

```
SetRegistryValue sans ACL       → équivaut à RunCommand via IFEO
```

Or `crates/ks-broker/src/main.rs` déclare, aujourd'hui :

```rust
SetRegistryValue { hive: String, path: String, name: String, value: String },
```

Quatre chaînes libres, aucune restriction. **C'est le verbe interdit, sous son
propre nom.** Le même fichier le rappelle d'ailleurs douze lignes plus bas, dans
son bloc « ce qui ne sera jamais ajouté ici » — la contradiction tient dans un
seul écran.

### Pourquoi c'est une exécution de code arbitraire, sans détour

La question éliminatoire du §7 est : *ce verbe permet-il, directement ou par
détour, d'exécuter du code arbitraire ?* Pour celui-ci, la réponse est oui, et
il suffit d'une écriture :

| Clé | Effet |
|---|---|
| `…\Image File Execution Options\<exe>\Debugger` | tout lancement de `<exe>` exécute le programme nommé, avec les droits de l'appelant |
| `…\Winlogon\Userinit` et `Shell` | code exécuté à chaque ouverture de session |
| `SYSTEM\CurrentControlSet\Services\<svc>\ImagePath` | code exécuté en tant que SYSTEM au démarrage |
| `…\Windows\AppInit_DLLs` | bibliothèque injectée dans les processus qui chargent `user32` |
| `…\CLSID\{…}\InprocServer32` | détournement de COM |

Un broker qui accepte `SetRegistryValue` sans contrainte **est** un exécuteur de
commandes. La barrière SEC-02 ne l'a pas vu, et c'est instructif : elle refuse
les variantes *nommées* comme une primitive d'exécution — `run`, `exec`, `cmd`,
`script`. Elle est aveugle à celle qui en est une sans le dire. Sa limite était
documentée pour `RunScript { path }` ; elle est plus large que documenté.

### Le second verbe, moins évident mais du même ordre

```rust
SetServiceStartup { service: String, startup: String },
```

Il n'exécute pas de code. Mais `service` est libre, donc
`SetServiceStartup { service: "WinDefend", startup: "disabled" }` est
`DisableDefender` écrit autrement — et le §7 exige que celui-ci « passe par la
convergence, avec diff, instantané et Windows Hello ». Un verbe atomique à
paramètre libre court-circuite les trois.

## Décision

**Aucun verbe du broker ne transporte de chemin, de nom de clé ou de nom de
service fourni par le client.** Les deux verbes concernés sont remplacés par des
variantes dont les paramètres sont des **énumérations fermées, sans champ**.

```rust
SetManagedSetting { setting: ManagedSetting, value: SettingValue },
SetServiceStartup { service: ManagedService, startup: StartupType },
```

Le client désigne *ce qu'il veut obtenir* ; c'est le broker qui détient la
correspondance vers la clé de registre ou le service. Ajouter un réglage
consiste à ajouter une variante dans le broker, donc à passer par une revue,
plutôt qu'à composer une chaîne côté appelant.

Les énumérations de paramètres sont **sans champ**, et un test le vérifie sur le
texte du source. Une variante unitaire ne peut, par construction, transporter ni
chemin ni valeur libre : c'est la même famille de barrière que le `match`
exhaustif, elle casse la compilation ou le test avant qu'un humain ait à y
penser.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| **Garder les verbes, valider les chemins par liste blanche à l'exécution** | Une liste blanche est une donnée ; une énumération est un type. La première se contourne par une faute de normalisation — `HKLM\SOFTWARE\..\..\SYSTEM\…`, casse mixte, chemin court 8.3, lien symbolique de clé. La seconde ne se contourne pas. Et la règle `rust.md` est explicite : rendre les états illégaux inconstructibles vaut mieux que les refuser à l'exécution. |
| **Garder les verbes et exiger Windows Hello à chaque appel** | Déplace la sécurité vers l'utilisateur, qui n'a aucun moyen de juger si `…\IFEO\notepad.exe\Debugger` est légitime. Une confirmation qu'on ne peut pas évaluer n'est pas une confirmation (P6). |
| **Les retirer sans rien mettre à la place** | La convergence a réellement besoin d'écrire des réglages en Phase 2. Retirer sans remplacer ferait réapparaître le besoin sous une forme improvisée, au pire moment. |
| **Attendre la Phase 2 pour trancher** | Les verbes sont déclarés et inertes : les corriger coûte zéro aujourd'hui. En Phase 2, quelqu'un les implémentera **tels qu'ils sont déclarés**, parce que le contrat de l'API est réputé figé. C'est précisément le moment où c'est le moins cher. |

## Les quatre questions, pour chacun des deux verbes

### `SetManagedSetting`

1. **Sait-il se simuler ?** Oui. Le broker connaît la clé cible, lit sa valeur
   actuelle et produit le diff avant / après. C'est même ce qui rend la
   correspondance obligatoire : un chemin libre ne se simule pas, faute de savoir
   ce qu'on est en train de changer.
2. **Sait-il s'annuler ?** Oui, par export préalable de la clé, joint à
   l'instantané. Le cas de la valeur absente avant écriture se restaure par une
   suppression, pas par l'écriture d'un zéro — la nuance est celle qu'a déjà
   coûtée le collecteur de posture.
3. **Exige-t-il une présence humaine ?** Oui pour tout réglage classé sécurité,
   au sens de SEC-08. Non pour les réglages de confort, dont la liste est fermée
   et revue.
4. **Est-il idempotent ?** Oui. Écrire la valeur déjà en place est un
   non-événement, et le diff vide le montre.

### `SetServiceStartup`

1. **Sait-il se simuler ?** Oui : lecture de `Start`, diff des deux types de
   démarrage traduits en français.
2. **Sait-il s'annuler ?** Oui, le type précédent est une valeur, conservée dans
   l'instantané.
3. **Exige-t-il une présence humaine ?** **Oui, systématiquement**, parce que la
   liste `ManagedService` ne contient que des services dont l'arrêt est un signal
   au sens du §6. Il n'existe pas de cas de confort ici.
4. **Est-il idempotent ?** Oui, même raisonnement.

**Question éliminatoire, pour les deux :** permettent-ils, directement ou par
détour, d'exécuter du code arbitraire ? **Non**, et c'est démontrable plutôt
qu'affirmé : le client ne peut nommer ni un chemin, ni un exécutable, ni un
service hors de la liste. L'ensemble des cibles atteignables est fini, énuméré
dans le source, et lisible par un relecteur en un écran.

## Conséquences

### Ce que ça nous donne

La barrière SEC-02 cesse d'être seulement nominale. Jusqu'ici elle refusait les
verbes *nommés* comme une primitive d'exécution ; elle refuse désormais aussi
ceux qui **en sont une** sans le dire, puisqu'aucun verbe ne peut plus recevoir
de cible arbitraire.

### Ce que ça nous coûte

Chaque réglage à écrire doit être ajouté au broker, avec sa correspondance et sa
classification sécurité. C'est du travail, et c'est le but : ce travail **est**
la revue.

### Ce que ça ferme

L'écriture de réglages non prévus, y compris légitimes. Un utilisateur qui veut
piloter une clé exotique devra la faire ajouter, ou s'en passer. C'est un choix
assumé : le §4.3 du cahier des charges annonce déjà que Keystone ne remplace pas
un éditeur de registre.

### Ce qui reste à faire

La liste des réglages gérés est **volontairement minimale** en Phase 0 : elle
sera peuplée en Phase 2, au fil des besoins réels de la convergence. Chaque
ajout se relit ici. Ce que cette ADR fige, c'est la **forme**, pas le contenu.
