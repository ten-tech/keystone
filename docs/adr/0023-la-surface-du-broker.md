# ADR-0023 — La surface du broker, ce qui la franchit et ce qui la garde

- **Statut** : Proposé le 2026-08-24, révisé le 2026-08-24 après trois relectures adverses
- **Date** : 2026-08-24
- **Exigences concernées** : SEC-01, SEC-02, SEC-03, SEC-04, SEC-07, SEC-08, SEC-09, SEC-10, SEC-11, SEC-12, D4-04, D11-02, D14-06, D16-03, P2, P3, P5, P6, P10
- **Remplace** : [ADR-0002](0002-grpc-sur-named-pipe.md), dont le statut passe à « Remplacé par ADR-0023 » dans le commit qui porte ce document, et qui n'est pas supprimée
- **Complète** : [ADR-0004](0004-chainage-du-journal.md), [ADR-0006](0006-fermer-les-verbes-a-parametres-libres.md), [ADR-0007](0007-ce-qui-garde-lenumeration-des-verbes.md), [ADR-0021](0021-ce-quun-instantane-sait-defaire.md)
- **Ce document n'ajoute aucun verbe.** Il en retire un paramètre (§ 28), et c'est le seul changement de la surface d'API qu'il porte. Les quatre questions du modèle de menace y figurent pour les **sept** verbes déjà déclarés, cinq d'entre eux ne les ayant jamais reçues.
- **Ce que la révision du 2026-08-24 change**, pour qui a lu la première rédaction : `deny_unknown_fields` vit sur `Verb` **et** sur l'enveloppe et non sur l'enveloppe seule (§ 3) ; le masque du client passe de `0x00100183` à `0x00120183`, `READ_CONTROL` étant accordé pour que le client sache à qui il parle (§ 4 et § 6) ; trois paramètres de création cessent d'être au défaut (§ 5) ; la fonction totale du § 17 couvre les cinq énumérations de paramètres et non une seule ; les codes de sortie se scindent en deux espaces et 82 se scinde en 82, 85 et 86 (§ 16) ; trois sections nouvelles apparaissent, § 29, § 30 et § 31 ; et la table des mesures est rejouée, M2 s'étant révélée fausse.

## Contexte

### Ce qui a déclenché ce document

Un audit adverse de `crates/ks-broker/src/main.rs`, mené le 2026-08-23, a produit
quatorze constats bloquants et six constats importants. Quatre positions
instruites, avec mesures, en ont tiré des réponses sur le transport, l'identité de
l'appelant, la présence humaine et l'intégrité. Le présent document les arbitre et
les fige.

Cinq constats commandent tous les autres, et il faut les lire avant la décision.

**Les quatre questions manquent pour cinq verbes sur sept.** Seuls
`SetServiceStartup` et `SetManagedSetting` les portent, dans l'ADR-0006. `Scan`,
`TakeSnapshot`, `RestoreSnapshot`, `AddDefenderExclusion` et `Isolate` ne les ont
jamais reçues, alors que l'ADR-0006 qualifie elle-même `RestoreSnapshot` de « verbe
le plus puissant de l'énumération ».

**La barrière garde la forme au moment où seul le contenu va grandir.** Les treize
tests de `ks-broker` interdisent qu'un verbe transporte une chaîne libre. Ils ne
disent rien d'une variante ajoutée à `ManagedSetting`, `ManagedService` ou
`SnapshotSubject`. Or l'ADR-0006 annonce explicitement que la liste des réglages
gérés « sera peuplée en Phase 2, au fil des besoins réels de la convergence », et
qu'elle fige « la **forme**, pas le contenu ». La surface qui va grandir est donc
exactement celle que rien ne garde.

**L'ADR-0002 décide un protocole dont deux justifications sont fausses à la
mesure**, et prescrit un « jeton par session » dont aucune position n'a su dire ce
qu'il apporterait.

**Le journal du broker n'existe pas, et sa forme actuelle ne peut pas porter
SEC-03.** `JournalEntry` compte **huit** champs (`crates/ks-core/src/journal.rs`,
lignes 169 à 185 : `seq`, `at`, `actor`, `verb`, `target`, `diff`, `outcome`,
`prev_digest`) et **aucun** ne porte les paramètres, que SEC-03 exige
nommément. Le magasin de la CLI, lui, vit sous `%LOCALAPPDATA%`
(`crates/ks-cli/src/magasin.rs:73`), c'est-à-dire à portée d'écriture de
l'adversaire A1, ce que le code reconnaît lui-même (`magasin.rs:628`).

**`VerbResult { simulated: bool }`** (`crates/ks-broker/src/main.rs`, lignes 438 à
440) est le booléen que `rust.md` proscrit nommément : un ordre entre deux étapes
porté par un drapeau plutôt que par le typage.

### L'état constaté, avec ses lignes

| Constat | Où | Effet |
|---|---|---|
| `Scan { domain: Option<String> }` | `ks-broker/src/main.rs:62` | chaîne libre destinée à un filtre d'affichage, sur un verbe qui s'exécutera en SYSTEM |
| `VerbResult { simulated: bool }` | `ks-broker/src/main.rs:438` | l'ordre simulation puis application n'est pas porté par le typage |
| `JournalEntry` sans paramètres | `ks-core/src/journal.rs:169` à `:185` | SEC-03 inapplicable en l'état |
| Magasin sous `%LOCALAPPDATA%` | `ks-cli/src/magasin.rs:73` et `ks-cli/src/main.rs:488` | à portée de A1 |
| `Actor::Human(String)` rempli depuis l'environnement | `ks-cli/src/main.rs:1049` et `:1071` | l'acteur journalisé est choisi par le processus parent |
| `plan.rs` porte `requires_presence: bool` | `ks-core/src/plan.rs:65` | un drapeau porté par l'appelant, qui ne doit jamais décider côté broker |
| `unsafe_code = "warn"` | `ks-broker/Cargo.toml:35` | déjà posé, et il faut dire d'avance ce qui l'emploiera |
| `deny = []` | `deny.toml:67`, avec le commentaire « quand ça le deviendra, la raison s'écrit ici » | l'endroit prévu pour interdire une crate nommément |

### Les mesures, et qui les a faites

Une première rédaction annonçait que « les mesures ci-dessous ont été **rejouées**
pour ce document », alors que M2, M3 et M4 ne portaient aucune commande mais la
mention « relevé de la position ». C'était le défaut que ce dépôt traque en
priorité, placé dans la table des mesures elle-même. Les trois ont donc été
réellement rejouées le 2026-08-24, hors dépôt, et **M2 était fausse** : le chiffre
corrigé figure ci-dessous. La colonne « Commande » dit désormais ce qui a été
exécuté ; toute case qui n'en porte pas est un relevé repris, et le dit.

`git status --porcelain` ne montre aucune modification de fichier suivi du fait de
ces relevés. Les mesures de coût en crates se font dans un projet jetable créé
hors du dépôt, puis par différence d'ensembles avec l'arbre de M1.

| # | Mesure | Commande | Résultat |
|---|---|---|---|
| M1 | Arbre de `ks-broker` | `cargo tree -p ks-broker -e normal --target x86_64-pc-windows-msvc --prefix none`, dédoublonné | **25 crates**, dont `blake3 1.8.5` et `windows-link 0.2.1` déjà présentes |
| M2 | Coût de gRPC utilisable | même commande sur un projet jetable portant `tonic = "0.14.6"`, puis différence avec M1 | **+51** crates pour `tonic` seul, **+55** avec `prost 0.14.4`, dont `axum`, `hyper`, `h2`, `httparse`, `tokio`, `tower` et `mio`. Plus `protoc`, binaire externe hors `Cargo.lock` |
| M2b | MSRV de `tonic` | `cargo info tonic` | `rust-version: 1.88`, soit **exactement** la MSRV du dépôt (`Cargo.toml:67`). Marge nulle |
| M3 | Coût de la vérification de signature | même commande sur un projet jetable portant `windows-sys` avec `Win32_Security`, `Win32_System_Pipes`, `Win32_Foundation` | **+1** crate (`windows-sys`), `windows-link` étant déjà là |
| M4 | Coût de WinRT dans le broker | même commande sur un projet jetable portant `windows = "0.62.2"` avec `Security_Credentials` et `Foundation` | **+10** noms de crates, et un **second `syn` majeur** dans le verrou : `windows-implement 0.62` tire `syn 2.0.119` quand `ks-broker` porte `syn 3.0.3`. Soit onze entrées de verrou pour dix noms |
| M5 | `ExclusionPath` face à une syntaxe d'interpréteur | copie du `TryFrom` du dépôt, hors dépôt, dix cas, recompilée et rejouée le 2026-08-24 | voir ci-dessous, reproduite à l'identique |
| M6 | ACL de `C:\ProgramData` | `icacls C:\ProgramData` | `BUILTIN\Utilisateurs:(OI)(CI)(RX)` **et** `BUILTIN\Utilisateurs:(CI)(WD,AD,WEA,WA)`, `CREATEUR PROPRIETAIRE:(OI)(CI)(IO)(F)` |
| M7 | ACL de `C:\Program Files` | `icacls 'C:\Program Files'` | `BUILTIN\Utilisateurs:(RX)`, `NT SERVICE\TrustedInstaller:(F)`, `CREATEUR PROPRIETAIRE:(OI)(CI)(IO)(F)`, aucune écriture pour `Utilisateurs` |
| M8 | `wrappers` de `cargo-deny` | `cargo deny check bans` sur une copie de `git archive HEAD` | syntaxe acceptée par cargo-deny 0.20.2, et `warning[unused-wrapper]` quand la dérogation ne sert pas |
| M9 | Suite du broker | `cargo test -p ks-broker` | 13 tests, tous verts |
| M10 | ACL d'un **sous-répertoire** de `C:\Program Files` créé par un installeur tiers | `icacls` sur cinq répertoires (`Android`, `Application Verifier`, `BraveSoftware`, `Docker`, `dotnet`) | **aucun ACE non hérité**, `BUILTIN\Utilisateurs:(I)(RX)` seulement, aucun droit pour un SID d'utilisateur. `CREATEUR PROPRIETAIRE` y descend en `(I)(OI)(CI)(IO)(F)`, donc toujours **à héritage seul** : il ne s'applique qu'à ce que son titulaire crée, et A1 ne crée rien là |
| M11 | Où agit `deny_unknown_fields` sur une énumération à étiquette interne | programme jetable, `serde 1.0.229` et `serde_json 1.0.151`, les versions du verrou | voir ci-dessous, § 3 |
| M12 | Arbre de `ks-cli`, et coût réel de WinRT côté client | `cargo tree -p ks-cli -e normal --target x86_64-pc-windows-msvc --prefix none`, dédoublonné, puis différence avec M4 | **103 crates**, et `windows 0.62.2` **y est déjà**, tiré par `wmi 0.17.3`. Les dix crates de M4 sont toutes présentes : le coût de WinRT côté client est de **zéro nom de crate** |

La mesure M5 est celle qui décide le § 21. Le validateur du dépôt, tel qu'il est
écrit aujourd'hui (`ks-broker/src/main.rs`, lignes 375 à 432), rend :

```
C:\a; Start-Process calc.exe   => ACCEPTE
C:\a|calc                      => ACCEPTE
C:\a`calc                      => ACCEPTE
C:\a\ncalc                     => ACCEPTE   (saut de ligne réel)
C:\a\0calc                     => ACCEPTE   (octet nul réel)
C:\a\u{202e}txt.exe            => ACCEPTE   (inversion de sens d'écriture)
C:\progra~1                    => ACCEPTE   (alias 8.3)
C:\Program Files               => refus     (répertoire système entier)
C:\Windows                     => refus
C:\                            => refus
```

Deux lectures, et la seconde est la plus instructive. Le validateur borne la
**sémantique Defender**, ce qui est son rôle et ce qu'il fait bien. Il ne borne ni
la syntaxe d'un interpréteur, ni la normalisation Win32 : « C:\progra~1 » passe
quand « C:\Program Files » est refusé, alors que les deux désignent le même
dossier. Une barrière qui refuse le nom long et accepte le nom court ne refuse
rien.

Et la mesure M11, qui décide le § 3. Une enveloppe portant
`deny_unknown_fields`, un `Verb` à étiquette interne (`tag = "verb"`), trois
verbes dont une variante de structure et deux variantes unitaires :

```
                                                      clef sur       clef sur Verb
                                                      l'enveloppe    ET l'enveloppe
variante de structure, champ inconnu DANS le verbe    ACCEPTE        refus
variante unitaire, champ inconnu DANS le verbe        ACCEPTE        ACCEPTE
champ inconnu au niveau de l'enveloppe                refus          refus
```

La lecture est nette, et elle change la portée de la décision : posée sur la seule
enveloppe, la clef **ne ferme rien du tout à l'intérieur du verbe**, pour les sept
verbes, variantes de structure comprises. La limite que le § 3 annonçait n'est
vraie que dans la forme que le § 9 ne décrivait pas.

### Ce que ce document ne peut pas mesurer

`ks-broker` n'existe pas. Aucune ligne de tuyau, de descripteur de sécurité, de
défi signé ou de journal privilégié n'est écrite. Tout ce qui suit est donc une
**spécification**, et chaque endroit où elle s'appuie sur un comportement du
système est marqué comme non mesuré. Les épreuves correspondantes se jouent dans
la [VM de labo](../06-VM-DE-LABO.md), jamais sur l'hôte, et l'intégration continue
ne peut en jouer aucune (§ « Ce qui doit casser »).

## Décision

### 1. Le transport est un tuyau nommé Windows en mode message, et rien d'autre ne cadre les messages

`PIPE_TYPE_MESSAGE` (0x00000004) avec `PIPE_READMODE_MESSAGE` (0x00000002). Le
noyau délimite les messages ; nous n'écrivons ni préfixe de longueur, ni
accumulation partielle, ni plafond arithmétique. Un message plus grand que le
tampon fixe du broker rend `ERROR_MORE_DATA` (234), et le broker refuse puis ferme
la connexion : **la borne sur la taille d'un message** est obtenue sans une ligne
de logique.

Une première rédaction écrivait ici « la borne de déni de service », au singulier
et sans qualificatif. C'était trop large d'exactement une famille : le mode
message borne la **taille**, jamais le **nombre**, ni la **durée**, ni
l'**occupation**. Un adversaire que la DACL admet, donc A1, n'a pas besoin d'un
message trop grand pour rendre Keystone injoignable ; il lui suffit d'occuper les
quatre instances et de se taire. Ce que cela coûte et ce qui le borne sont écrits
au § 29, qui existe pour cette raison.

Ce que le mode message n'apporte pas non plus, et qu'il ne faut pas lui prêter : un
écrivain hostile émet parfaitement un message court et complet. Le refus vient
alors du décodeur, pas du transport. Les deux couches sont nécessaires, aucune ne
suffit.

### 2. Le codage est du JSON désérialisé directement en `Verb`, et gRPC est écarté

`serde_json` est déjà dans l'arbre du broker (M1). gRPC coûterait **51 crates**
pour `tonic 0.14.6` seul, et **55** avec `prost`, c'est-à-dire dans la forme où
il sert (M2), plus `protoc`, binaire externe hors `Cargo.lock`, donc hors
`cargo deny check`, hors `cargo audit`, et hors du verrouillage que le dépôt
s'impose partout ailleurs. Le chiffre précédemment écrit ici, 49, était repris
d'une position et n'avait pas été rejoué ; il était faux.

S'y ajoute une contrainte que la première rédaction ne voyait pas :
`tonic 0.14.6` déclare `rust-version = "1.88"` (M2b), **exactement** la MSRV du
dépôt (`Cargo.toml:67`). La marge est nulle : la première version de `tonic` qui
relève sa MSRV relève la nôtre, donc ferme la porte à des postes plus anciens,
par un `cargo update` que personne ne relit comme une décision. Le job MSRV de la
CI l'aurait révélé sans compiler ; c'est précisément ce qu'on n'a pas envie de
découvrir ainsi.

La raison décisive n'est pourtant pas le compte. **proto3 emploie des énumérations
ouvertes** : « In languages that support open enum types with values outside the
range of specified symbols, such as C++ and Go, the unknown enum value is simply
stored as its underlying integer representation »
([guide proto3](https://protobuf.dev/programming-guides/proto3/), consulté le
2026-08-24). Une précision de la même page, parce que la première rédaction disait
faux sur ce point : proto3 ne jette pas les champs inconnus, il les **conserve**,
« Proto3 messages preserve unknown fields and include them during parsing and in
the serialized output, which matches proto2 behavior ». C'est `prost` qui les
écarte, pas la spécification. La conclusion ne bouge pas, l'ouverture des
énumérations restant vraie et suffisante. Or toute la barrière SEC-02 repose sur la phrase que porte
`ManagedService` dans le source (`ks-broker/src/main.rs:133`) : « l'ensemble des
cibles atteignables est fini, énuméré ici, et lisible par un relecteur en un
écran ». Un codec dont l'ensemble des valeurs est ouvert par spécification ne peut
pas porter ce contrat, et cela ne se corrige pas en discipline de code.

Trois des crates que gRPC apporterait sont un serveur HTTP/2 complet et son
analyseur d'en-têtes, dans un binaire dont SEC-12 promet qu'il n'écoute sur aucun
port.

L'ADR-0007 a différé `syn` pour un coût en crates **nul**, au motif qu'il
« améliore l'auto-inspection du produit, pas le produit ». Si cet argument tient,
celui-ci tient a fortiori.

### 3. `deny_unknown_fields` est obligatoire sur `Verb` **et** sur l'enveloppe, et sa limite est écrite

La clef n'est pas dans `CLEFS_SERDE_ADMISES` (`ks-broker/src/main.rs:1011`, qui
admet `rename_all`, `tag` et `try_from`). Son inscription est un geste visible,
justifié ici : elle appartient à la même famille que `try_from`, c'est-à-dire aux
clefs qui **ajoutent** un contrôle au lieu d'en retirer un. Ce n'est pas un
assouplissement de la liste blanche, c'est son extension au seul motif qui la
justifie.

**Sur quel type elle vit est la décision, pas un détail de rédaction.** Une
première rédaction écrivait « sur le type de requête » sans nommer le type, et le
§ 9 ne posait la clef que sur l'enveloppe : deux protections différentes selon le
lecteur. La mesure M11 tranche, et elle est nette. Posée sur la seule enveloppe,
la clef laisse passer un champ inconnu **à l'intérieur** de l'objet du verbe, pour
les sept verbes, variantes de structure comprises : elle ne ferme alors que le
niveau où personne n'attaque. Posée sur `Verb`, elle refuse les variantes de
structure et laisse passer les unitaires.

La clef est donc obligatoire **aux deux niveaux**, et le test l'exige aux deux :
un champ inconnu dans l'enveloppe est refusé, un champ inconnu dans une variante
de structure est refusé, un champ inconnu dans une variante unitaire est accepté
et jeté. Trois assertions, pas une.

Sa limite, mesurée : `deny_unknown_fields` sur `Verb` ferme les variantes **de
structure**, pas les variantes **unitaires**.
`{"verb":"isolate","cmd":"calc.exe"}` est accepté, le champ étant jeté. Rien ne
s'exécute, et le journal ne le verra jamais (§ 19), mais le fait se documente par
un test qui décrit le comportement réel et deviendra rouge, donc visible, si serde
change.

**Ce que la liste blanche des clefs serde n'attrape pas, et pourquoi c'est admis
ici.** `CLEFS_SERDE_ADMISES` est indexée par nom seul, quand ses deux voisines
`CHAMPS_TEXTE_ADMIS` est indexée par couple `(variante, champ)` et
`TYPES_VALIDES_ADMIS` par triplet `(variante, champ, type)`, sa quatrième colonne
portant la raison. Les deux le sont depuis le
resserrement du 2026-08-23, avec l'argument qu'une exemption par nom vaut alors
pour tout type. L'argument ne se transpose pas, et il faut dire pourquoi plutôt
que de recopier le geste. Ces deux listes-là enregistrent des **exemptions** :
elles autorisent ce qui serait sinon refusé, donc leur portée est une surface.
Celle-ci enregistre des **obligations** : `try_from` et `deny_unknown_fields`
n'élargissent jamais ce qu'un client peut envoyer, quel que soit le type qui les
porte ; elles le restreignent. Une clef inscrite ici ne peut donc pas devenir une
porte en migrant vers un autre type. Ce qui manquerait, en revanche, c'est la
**présence** de la clef là où elle sert : le test l'exige nommément sur `Verb` et
sur l'enveloppe, comme `un_type_valide_ne_se_contourne_pas` exige déjà `try_from`
sur chacun des trois types validés.

### 4. La DACL du tuyau est composée par Keystone, en droits individuels, et la voici

Le descripteur par défaut est faux pour nous, et la documentation le dit sans
détour : « If *lpSecurityAttributes* is **NULL**, the named pipe gets a default
security descriptor [...] The ACLs in the default security descriptor for a named
pipe grant full control to the LocalSystem account, administrators, and the
creator owner. They also grant read access to members of the Everyone group and
the anonymous account »
([CreateNamedPipeA](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea),
consulté le 2026-08-24).

```
O:SYG:SYD:P(A;;0x001f01ff;;;SY)(A;;0x00120183;;;{LOGON_SID})S:(ML;;NRNWNX;;;ME)
```

`{LOGON_SID}` est substitué à l'exécution par la forme rendue par
`ConvertSidToStringSidW`.

Le masque du client, `0x00120183`, se décompose ainsi, et **il est écrit en
hexadécimal exprès** :

| Droit | Valeur | Pourquoi |
|---|---|---|
| `FILE_READ_DATA` | `0x0001` | lire la réponse |
| `FILE_WRITE_DATA` | `0x0002` | émettre la trame |
| `FILE_READ_ATTRIBUTES` | `0x0080` | `GetNamedPipeInfo` |
| `FILE_WRITE_ATTRIBUTES` | `0x0100` | `SetNamedPipeHandleState`, pour que le client passe **sa propre** poignée en mode message |
| `READ_CONTROL` | `0x00020000` | **lire le propriétaire du tuyau**, sans quoi le client ne peut pas savoir à qui il parle (§ 6) |
| `SYNCHRONIZE` | `0x00100000` | attendre sur la poignée |

Et ce qui est refusé compte davantage :

| Refusé | Valeur | Conséquence s'il était accordé |
|---|---|---|
| `FILE_APPEND_DATA`, identique à `FILE_CREATE_PIPE_INSTANCE` | `0x0004` | **squattage du nom** |
| `WRITE_DAC`, `WRITE_OWNER`, `DELETE` | `0x00040000`, `0x00080000`, `0x00010000` | réécriture, reprise de propriété, destruction du canal |
| tout mnémonique agrégé (`GA`, `GR`, `GW`, `GX`, `FA`, `FR`, `FW`, `FX`) | sans objet | non relu, et `FW` contient `0x0004` |

Ce dernier point est la raison d'être du tableau, et il vient de la source : « Because
FILE\_APPEND\_DATA and FILE\_CREATE\_PIPE\_INSTANCE have the same definition, so
FILE\_GENERIC\_WRITE enables permission to create the pipe. To avoid this problem,
use the individual rights instead of using FILE\_GENERIC\_WRITE »
([Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights),
consulté le 2026-08-24). Un descripteur écrit avec `FW` accorde le squattage à qui
il prétend accorder l'écriture.

`D:P` protège la DACL : aucun héritage, donc `Everyone` et le compte anonyme
disparaissent par construction, sans qu'il faille poser d'ACE de refus, dont
l'ordre est une occasion de se tromper. **Aucun ACE pour `BA`** : l'utilisateur
interactif d'un poste Windows 11 est en général membre des administrateurs, mais
son jeton non élevé porte ce SID en refus seul ; un ACE `BA` ne servirait pas la
CLI et élargirait la surface pour rien. L'ACE `SY` porte `FILE_ALL_ACCESS`
(`0x001f01ff`) parce que la même page l'exige : « In addition to the requested
access rights, the DACL must allow the calling thread FILE\_CREATE\_PIPE\_INSTANCE
access to the named pipe. »

**`READ_CONTROL` est accordé, et c'est un renversement de la première
rédaction**, qui le refusait au motif que lire la DACL est de la reconnaissance.
Deux raisons, dont la seconde est décisive. La reconnaissance ainsi refusée
n'existe pas : la DACL est écrite en toutes lettres dans le présent document, qui
est public ; la cacher à A1 ne lui dissimule rien qu'il ne puisse lire ici. Et la
refuser fermait la **seule** vérification bon marché dont le client disposait :
« To read the owner, group, or DACL from the object's security descriptor, the
calling process must have been granted READ\_CONTROL access when the handle was
opened »
([GetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo),
consulté le 2026-08-24). Sans ce droit, `ks` ne peut pas constater que le tuyau
qu'il vient d'ouvrir appartient à `SYSTEM`, donc ne sait pas à qui il parle. Deux
décisions individuellement défendables s'annulaient l'une l'autre ; c'est celle
qui coûte le moins qui cède. L'usage qu'en fait le client est écrit au § 6.

`S:(ML;;NRNWNX;;;ME)` pose une étiquette d'intégrité obligatoire au niveau moyen.
Elle ne protège **pas** contre A1, qui s'exécute au même niveau. **Et elle apporte
moins que ce que la première rédaction lui prêtait**, ce qui se mesure à la
source : « Any object without an integrity SID is treated as if it had medium
integrity », et « By default, the system creates every object with an access mask
of SYSTEM\_MANDATORY\_LABEL\_NO\_WRITE\_UP »
([Mandatory Integrity Control](https://learn.microsoft.com/en-us/windows/win32/secauthz/mandatory-integrity-control),
consulté le 2026-08-24). Le cas « un script dans un onglet de navigateur », que
l'ADR-0002 citait, était donc **déjà fermé par le défaut** : un moteur de rendu
s'exécute en intégrité basse ou en conteneur d'application, il doit écrire pour
demander quoi que ce soit, et `NO_WRITE_UP` s'applique sans qu'on écrive rien.
L'apport réel de l'étiquette explicite est `NR` et `NX`, pas `NW`. La conclusion
tient ; sa justification était plus faible qu'annoncée, et une justification
fausse sous une conclusion juste est exactement ce qu'une revue ultérieure croira.

Le logon SID est le seul mécanisme disponible pour séparer deux sessions du même
utilisateur **sans changer le nom du tuyau**, et c'est la page ci-dessus qui
l'indique : « To prevent remote users or users on a different terminal services
session from accessing a named pipe, use the logon SID on the DACL for the pipe. »
L'autre voie, un nom de tuyau par session, est examinée et écartée dans les
alternatives.

**Non mesuré, et nommé pour l'épreuve de labo** : la présence et la lisibilité du
logon SID sur le poste de référence, le fait qu'un descripteur portant une
étiquette d'intégrité soit accepté par `CreateNamedPipeW` sans privilège
supplémentaire, et le comportement de `SetNamedPipeHandleState` depuis une poignée
ouverte avec ces droits individuels.

Ce dernier point mérite d'être écrit sans l'adoucir, parce que la résolution
annoncée par la première rédaction, « élargir le masque des seuls bits mesurés
nécessaires », pouvait signifier « accorder le squattage ». La source dit : « The
handle must have GENERIC\_WRITE access to the named pipe for a write-only or
read/write pipe, or it must have GENERIC\_READ and FILE\_WRITE\_ATTRIBUTES access
for a read-only pipe »
([SetNamedPipeHandleState](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-setnamedpipehandlestate),
consulté le 2026-08-24). Notre tuyau est `PIPE_ACCESS_DUPLEX`, donc read/write,
donc la voie **documentée** est `GENERIC_WRITE`, qui contient `FILE_APPEND_DATA`,
c'est-à-dire `FILE_CREATE_PIPE_INSTANCE`, c'est-à-dire le squattage que le tableau
ci-dessus refuse. Les deux exigences se contredisent.

**La contradiction se tranche d'avance, et dans ce sens : `0x0004` n'est jamais
accordé, quelle que soit la mesure.** Si le labo établit que
`SetNamedPipeHandleState` ne fonctionne pas depuis les droits individuels de ce
masque, le client ne passe pas sa poignée en mode message, et le repli est écrit
ici plutôt qu'improvisé : **une seule requête par connexion**, le broker écrivant
sa réponse puis fermant. La fin du message devient la fin du flux, que le client
lit en mode octet sans avoir à en connaître la frontière. Le coût est une
connexion par échange, ce qui est négligeable pour cette charge, et il est
préférable à un bit qui donne le nom du canal. Le repli est un mode nominal
admissible, pas une dégradation : il est décrit ici pour que la mesure ne puisse
pas être arbitrée par la voie de moindre résistance le jour où elle tombe.

### 5. Les drapeaux de création sont écrits, et aucun n'est laissé au défaut

```
dwOpenMode    = PIPE_ACCESS_DUPLEX (0x00000003)
              | FILE_FLAG_FIRST_PIPE_INSTANCE (0x00080000)   [première instance seulement]
dwPipeMode    = PIPE_TYPE_MESSAGE (0x00000004)
              | PIPE_READMODE_MESSAGE (0x00000002)
              | PIPE_WAIT (0x00000000)
              | PIPE_REJECT_REMOTE_CLIENTS (0x00000008)
nMaxInstances   = 4, jamais PIPE_UNLIMITED_INSTANCES
nOutBufferSize  = 64 Kio, écrit
nInBufferSize   = 64 Kio, écrit
nDefaultTimeOut = 5 000 ms, écrit
```

`PIPE_ACCEPT_REMOTE_CLIENTS` vaut `0x00000000` : **accepter les clients distants
est ce qu'on obtient en n'écrivant rien**. C'est le genre de défaut qu'une revue
ne voit pas, puisqu'il n'y a rien à lire.

**Les trois derniers paramètres sont écrits parce que le titre de cette section le
promet.** Une première rédaction s'intitulait « aucun n'est laissé au défaut » et
n'en écrivait que trois sur huit. Les trois manquants ne sont pas décoratifs.
`nOutBufferSize` et `nInBufferSize` sont pris sur le pool non paginé, « Every time
a named pipe is created, the system creates the inbound and/or outbound buffers
using nonpaged pool, which is the physical memory used by the kernel », et c'est
leur valeur qui décide où un message trop grand devient `ERROR_MORE_DATA`,
c'est-à-dire la borne que le § 1 dit obtenir sans une ligne de logique : la borne
existe parce que ce nombre est écrit. `nDefaultTimeOut` appartient aux valeurs qui
doivent être identiques entre instances, et il porte un défaut que personne n'a
choisi : « A value of zero will result in a default time-out of 50 milliseconds »
([CreateNamedPipeA](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea),
consulté le 2026-08-24). Les valeurs ci-dessus sont des **jugements et non des
mesures**, et ce mot est écrit ici comme il l'est pour l'échéance des défis : le
labo les révisera contre la taille réelle d'un diff de convergence.

Trois pièges à écrire, parce qu'ils se paient à l'exécution et non à la revue.
`FILE_FLAG_FIRST_PIPE_INSTANCE` vaut `0x00080000`, la même valeur numérique que
`WRITE_OWNER` dans un autre paramètre. Le drapeau se pose sur la **première**
instance seulement, les suivantes échouant sinon. Et **chaque instance porte son
propre descripteur de sécurité** : la DACL se fournit à chaque appel, pas au
premier.

**Et un quatrième, que la première rédaction ne voyait pas : le noyau n'impose pas
la cohérence de tous ces paramètres entre instances.** La source distingue
nettement. Doivent être identiques : « All instances of a named pipe must specify
the same pipe type (byte-type or message-type), pipe access (duplex, inbound, or
outbound), instance count, and time-out value. If different values are used, this
function fails and GetLastError returns ERROR\_ACCESS\_DENIED. » Peuvent différer :
« One of the following remote-client modes can be specified. Different instances
of the same pipe can specify different remote-client modes », et la même formule
vaut pour le mode de lecture et le mode d'attente (même page, même date). Une
instance créée sans `PIPE_REJECT_REMOTE_CLIENTS`, ou sans `PIPE_READMODE_MESSAGE`,
ne produit donc **aucune erreur** : elle est seulement plus faible que ses sœurs,
et rien ne le signale.

D'où une contrainte de code, et non une consigne : **les huit paramètres vivent
dans une seule constante, et toutes les instances passent par la même fonction**,
qui n'accepte en argument que le rang de l'instance, lequel décide du seul drapeau
qui varie légitimement, celui de première instance. Un test compare la constante
au tableau ci-dessus, champ par champ. La barrière annoncée « casse un test »
couvrait le SDDL caractère par caractère et pas les drapeaux de la n-ième
instance ; elle les couvre désormais.

### 6. `ERROR_ACCESS_DENIED` à la création de la première instance vaut refus de démarrer

Pas de repli sur un autre nom, pas de nouvel essai, pas de démarrage dégradé. Le
broker écrit une entrée de journal privilégié, rend un code de sortie distinct
réservé à « canal usurpé », et affiche un message à trois temps.

La lecture opérationnelle demande une précision que la source impose. Deux causes
produisent `ERROR_ACCESS_DENIED` : le drapeau de première instance face à un nom
déjà pris, et un désaccord de paramètres entre instances, la documentation exigeant
que « All instances of a named pipe must specify the same pipe type [...] instance
count, and time-out value. If different values are used, this function fails and
GetLastError returns **ERROR\_ACCESS\_DENIED** ». Sur la **première** instance,
aucune instance à nous n'existe encore : la seconde cause suppose alors qu'un
autre processus détient déjà le nom. La conclusion tient donc, et elle ne tient
que là. Sur les instances suivantes, le même code peut signifier notre propre
incohérence de paramètres, et le code doit distinguer les deux appels.

**Cette lecture est une déduction, jamais une citation, et il ne faut pas la
présenter autrement.** La documentation dit seulement : « If you attempt to create
multiple instances of a pipe with this flag, creation of the first instance
succeeds, but creation of the next instance fails with ERROR\_ACCESS\_DENIED »
([CreateNamedPipeA](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea),
consulté le 2026-08-24) : la phrase vise **vos** tentatives, pas celles d'un
tiers. Le comportement attendu face à un squatteur se déduit du contrôle d'accès,
et c'est l'épreuve 2 du laboratoire. Ce qui **est** documenté, en revanche, et qui
tient le régime permanent : « To create an instance of a named pipe by using
CreateNamedPipe, the user must have FILE\_CREATE\_PIPE\_INSTANCE access to the
named pipe object » (même page). Le refus de `0x0004` dans le masque du client
interdit donc d'ajouter une instance à un tuyau que le broker détient déjà.

Le mode lecture seule de SEC-07 **ne s'applique pas ici** : il couvre l'échec d'un
contrôle d'intégrité, où le broker sait encore qui il est. Ici il ne sait plus s'il
est joignable, et démarrer en lecture seule ajouterait un second répondeur au
premier, en laissant croire que l'un des deux est le bon.

**Ce que ce refus fait, et ce qu'il ne fait pas.** Une première rédaction écrivait
que démarrer en lecture seule « laisserait l'usurpateur répondre aux lectures ».
La justification était inversée, et l'inversion est instructive : refuser de
démarrer n'empêche **rien** à l'usurpateur, qui détient le nom et répondra de
toute façon. Ce que le refus garantit, c'est qu'il soit **seul** à répondre, donc
qu'aucune ambiguïté ne subsiste sur ce qui parle. Le refus est une condition de
lisibilité, pas une parade.

La parade est ailleurs, et elle est due côté client, sans quoi tout ce paragraphe
protège l'usurpateur plutôt que l'utilisateur. Service arrêté, en échec, ou pas
encore installé, A1 tient le nom : `ks` et la coque lui parlent, et il dicte ce
que l'utilisateur croit de la posture de sa machine, ce qui est la valeur entière
du produit. **Le client vérifie donc à qui il parle avant d'écrire quoi que ce
soit** : après `CreateFileW`, il appelle `GetSecurityInfo` sur la poignée avec
`OWNER_SECURITY_INFORMATION`, que le masque `0x00120183` rend possible (§ 4), et
refuse si le propriétaire n'est pas `SYSTEM`. Le mécanisme tient parce qu'un
processus non élevé ne peut pas se donner `SYSTEM` pour propriétaire d'un objet
qu'il crée : il ne porte ce SID ni dans ses propriétaires possibles, ni par un
privilège. Refus, message à trois temps, code de sortie distinct (§ 16), et rien
n'est envoyé.

Ce que cette vérification **ne** donne pas, et il faut l'écrire au même endroit :
elle dit que le tuyau appartient à `SYSTEM`, jamais qu'il appartient à *notre*
service. Un autre service élevé du poste, ou A2, la passe sans difficulté. Contre
A1, qui est l'adversaire visé ici, elle suffit ; contre A2, rien ne suffit et le
modèle de menace l'assume déjà.

La fenêtre de squattage se ferme par l'ordre : le service prend le nom **au
démarrage de la machine**, avec une DACL ne portant que l'ACE `SY`, puis ajoute
l'ACE de session à l'ouverture de session interactive et le retire à sa fermeture.
Coût assumé : le tuyau sert **une seule session interactive à la fois**. La bascule
rapide d'utilisateur n'est pas servie ; sur un poste personnel, périmètre déclaré
du produit, ce n'est pas une régression.

**Et le retrait de l'ACE ne suffit pas seul**, parce qu'il ne parle que des
connexions futures : le contrôle d'accès a lieu à l'ouverture, « when a client
calls the CreateFile or CallNamedPipe function to connect to the client end of a
named pipe, the system performs an access check before granting access to the
client »
([Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights),
consulté le 2026-08-24), et rien n'indique qu'une poignée déjà ouverte soit
réévaluée. Une connexion ouverte pendant la session survivrait donc à sa
fermeture, et le broker continuerait de la servir sous un logon SID qui n'a plus
cours, l'identité étant délibérément immuable pour la durée de la connexion
(§ 7). À la fermeture de session, le broker **ferme donc lui-même** toute
connexion dont l'identité capturée porte le logon SID retiré. **Non mesuré** : la
non-réévaluation d'une poignée ouverte, qui se déduit du modèle documenté du
contrôle à l'ouverture et n'est affirmée nulle part. C'est l'épreuve 9 du
laboratoire.

### 7. L'identité de l'appelant se dérive du jeton du système, jamais du message, et sa limite s'affiche dans le produit

La séquence, après lecture d'une trame complète : `ImpersonateNamedPipeClient`,
dont **le retour est vérifié sans exception**, puis `OpenThreadToken`, puis
`GetTokenInformation` pour l'utilisateur et pour le groupe portant
`SE_GROUP_LOGON_ID`, puis `RevertToSelf` dans un garde dont le `Drop` le rappelle
même en cas de panique.

La vérification du retour n'est pas de l'hygiène. La source l'écrit : « If the
**ImpersonateNamedPipeClient** function fails, the client is not impersonated, and
all subsequent client requests are made in the security context of the process
that called the function. If the calling process is running as a privileged
account, it can perform actions that the client would not be allowed to perform »
([ImpersonateNamedPipeClient](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-impersonatenamedpipeclient),
consulté le 2026-08-24). C'est le seul endroit de ce document où une valeur de
retour ignorée donne SYSTEM à l'appelant. La même page précise que le contexte
usurpé est celui « of the last message read from the pipe » : l'identité se capture
une fois, à l'acceptation, et devient une valeur immuable pour la connexion.

Côté client, `CreateFileW` porte `SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION`.
Il faut dire dans quel sens ce drapeau protège : il protège **le client** d'un
broker usurpé, qui pourrait sinon se faire passer pour lui ailleurs. Il n'apporte
rien au broker, un client hostile passant le drapeau qu'il veut.

**Et il faut décider ce que le broker fait quand le client choisit le cas
dégénéré**, parce que le choix lui appartient : « The client of a named pipe, RPC,
or DDE connection can control the impersonation level », et `SecurityAnonymous`
donne « The server cannot impersonate or identify the client »
([Impersonation Levels](https://learn.microsoft.com/en-us/windows/win32/secauthz/impersonation-levels),
consulté le 2026-08-24). Le contrôle d'accès de la DACL a bien eu lieu à
l'ouverture, avec le vrai jeton ; mais l'usurpation qui suit peut réussir et ne
nommer personne, et `SECURITY_EFFECTIVE_ONLY` relève du même geste. Trois
mécanismes reposent sur cette identité : l'attribution au journal (§ 19),
l'indexation de la limitation de débit (§ 23) et la liaison du défi (§ 10). Un
broker qui les ferait reposer sur « personne » les annulerait tous les trois en
silence.

**La dérivation d'identité a donc un mode d'échec nommé, et il refuse.** Si
`GetTokenInformation` ne rend pas un SID d'utilisateur exploitable, ou si le
niveau d'usurpation obtenu est inférieur à `SecurityIdentification`, le broker
refuse la connexion, la journalise sous une identité explicitement inconnue, et
rend un code distinct. Ce n'est pas une valeur par défaut, c'est une variante de
l'énumération d'échec que le `match` exhaustif du § 17 oblige à traiter. La
première rédaction rangeait ce cas dans les épreuves de laboratoire ; une épreuve
de laboratoire n'est pas une décision, et le comportement attendu doit être écrit
avant que la mesure le confirme.

**La limite, noir sur blanc, et elle s'affiche dans `ks journal` sous chaque ligne
d'appelant :**

> Dans la même session et sous le même utilisateur, `ks` et un binaire de
> l'adversaire A1 portent le même SID et le même logon SID. Le système ne sait pas
> les distinguer, et le broker non plus. Cette ligne dit **qui**, jamais **quel
> programme**.

La DACL est donc une frontière contre les autres utilisateurs, les autres sessions,
les clients distants et les processus d'intégrité inférieure. Elle n'est **pas**
une frontière contre A1, qui est l'adversaire numéro un du modèle de menace. Ce qui
tient contre A1 est ailleurs : l'ensemble fermé des verbes, la signature de
présence humaine sur l'empreinte du diff simulé, la limitation de débit persistée,
et le journal privilégié hors de sa portée.

**Toute phrase d'ADR ou de documentation laissant entendre que « les ACL du
système » protègent de A1 est fausse et se corrige** (§ « Ce que ça corrige dans la
documentation »).

Trois pistes d'identification sont écartées nommément, pour que la tentation ne
revienne pas avec de bonnes intentions : le PID de l'appelant, qui nomme un
processus et jamais le code qui s'y exécute ; le chemin de l'image, qu'une copie
suffit à falsifier et qu'un binaire authentique dans lequel A1 a écrit rend faux
sans même le falsifier ; et la signature du binaire appelant, qui affirme quelque
chose d'un fichier sur le disque, pas de la mémoire du processus, et qui ajouterait
`WinVerifyTrust` sur le chemin chaud du composant privilégié, piloté par un chemin
dérivé d'un identifiant choisi par l'attaquant.

### 8. Le jeton de session n'existe pas, et son refus est définitif plutôt que différé

L'argument tient en une phrase : **tout secret qu'un `ks` non élevé sait lire, A1
sait le lire**, puisqu'ils sont le même utilisateur dans la même session. Cela vaut
aussi pour la variante la plus tentante, celle d'un jeton émis par le broker après
une authentification forte : un porteur remis à `ks` est un porteur remis à A1.

Ce qui subsiste sous ce nom est un **identifiant de connexion opaque, généré par le
broker, sans aucune autorité**, dont l'unique fonction est de corréler les entrées
d'un même dialogue dans le journal. Il ne prouve rien, ne donne rien, ne se vérifie
pas, et n'a donc ni révocation ni comportement en cas de fuite : c'est la réponse
par la négative que la contrainte autorisait.

Le travail que le jeton prétendait faire va ailleurs, et il faut le dire pour que
la conclusion soit nette plutôt que négative :

| Ce qu'on attendait du jeton | Qui le fait réellement |
|---|---|
| prouver que l'appelant est `ks` | personne, et c'est impossible contre A1 (§ 7) |
| écarter un autre utilisateur, une autre session | la DACL : SID de l'utilisateur **plus** logon SID |
| écarter un client anonyme ou distant | refus à l'acceptation, et `PIPE_REJECT_REMOTE_CLIENTS` |
| lier une simulation à son application | le typestate et l'empreinte du diff simulé (§ 9 et § 10) |
| refuser un client de version incompatible | un champ de version en clair, refusé explicitement |
| limiter le débit par client | un compteur persistant indexé sur le SID (§ 23) |
| corréler un dialogue au journal | l'identifiant de connexion ci-dessus |

### 9. L'enveloppe porte une phase, jamais un verbe de plus, et l'ordre est un typestate

La requête est `{ "protocol": "…", "phase": "…", "verb": { "verb": "…", … } }`, avec
`deny_unknown_fields` sur l'enveloppe **et sur `Verb`**, les deux étant nécessaires
et aucune suffisante (§ 3 et M11). `phase` est une énumération **fermée à deux
variantes**, `simulate` et `apply`, sans champ ni paramètre. Elle n'ajoute aucune
capacité : chaque phase est une projection des mêmes sept verbes.

L'enveloppe ne porte **aucun champ d'identité**. Pas « il est validé » : il n'existe
pas dans le type. Aucun `actor`, `user`, `caller`, `sid`, `token` ni `session`.

Côté Rust, `VerbResult { simulated: bool }` disparaît au profit d'un typestate :

```rust
pub struct Plan<E> { /* champs privés */ }
pub enum Simulated {}
pub enum Approved {}

impl Plan<Simulated> {
    /// Consomme le plan et la preuve. Seul constructeur de `Plan<Approved>`.
    pub fn approve(self, preuve: PresenceProof) -> Result<Plan<Approved>, Refus>;
}
impl Plan<Approved> {
    pub fn apply(self, …) -> Result<Applied, Refus>;
}
```

`apply` n'existe que sur `Plan<Approved>`, dont il n'y a pas d'autre constructeur.
`PresenceProof` a un champ privé, aucun constructeur public hors du module de
vérification, et n'implémente **ni `Clone` ni `Copy`** : une preuve n'approuve pas
deux plans. `approve` prend `self` par valeur : un plan ne s'approuve pas deux fois.

Et une contrainte de lecture, découverte dans `ks-core` : le broker **ne lit jamais
`Action::requires_presence`** (`crates/ks-core/src/plan.rs:65`). Ce champ est
positionné par le module qui décrit l'action, donc par un composant non privilégié.
Il reste utile au client pour prévenir l'utilisateur en amont ; il n'a aucune
autorité. L'exigence de présence se calcule dans le broker, à partir du verbe, et
de nulle part ailleurs.

### 10. La présence humaine est une signature sur l'empreinte du diff simulé, vérifiée juste avant l'écriture

Aucun booléen de présence ne traverse jamais le tuyau.
`UserConsentVerifier.RequestVerificationAsync` rend un
`UserConsentVerificationResult` **au processus appelant** : un booléen rapporté par
le client est contrefait en une ligne par A1, qui porte le même SID. Et le broker,
service en session 0, ne peut de toute façon pas afficher d'invite : « Services
cannot directly interact with a user as of Windows Vista »
([Interactive Services](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services),
consulté le 2026-08-24 ; c'était la seule citation de ce document sans lien ni
date).

Le protocole, dans l'ordre exact :

1. le client ouvre la connexion ; le broker dérive le SID **après lecture de la
   première trame complète** et le fige pour la connexion (§ 7). L'identité ne
   peut pas se dériver à l'acceptation, le contexte usurpé étant celui « of the
   last message read from the pipe » : une première rédaction écrivait les deux,
   et les deux ne peuvent pas être vraies ;
2. le client demande la phase `simulate` sur un verbe ;
3. le broker simule, produit le diff lisible, et calcule `plan_digest`, empreinte
   BLAKE3 du matériau canonique du plan ; **BLAKE3 est déjà dans l'arbre**
   (M1) et déjà l'empreinte du journal (`ks-core/src/journal.rs:269`) ;
4. le broker range le plan sous un identifiant, lié à **cette** connexion, avec une
   échéance, et rend au client le diff, l'identifiant et un **défi** opaque ;
5. le client **affiche le diff** et attend que l'humain le lise (P6) ;
6. le client fait signer les octets du défi, **reçus tels quels et jamais
   reconstruits**, par `KeyCredential`, qui est « an RSA, 2048-bit, asymmetric key »
   ([KeyCredential](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.keycredential),
   consulté le 2026-08-24) ; Windows affiche l'invite, l'humain fait le geste ;
7. le client demande la phase `apply` avec l'identifiant et la signature ;
8. le broker **retire** le défi de sa table, de façon atomique ; absent, il refuse ;
9. le broker vérifie la signature sur les octets **qu'il avait stockés**, avec la
   clé publique enrôlée ; échec, il refuse ;
10. le broker **re-simule** contre l'état courant et recompare `plan_digest` ; si
    l'état a bougé pendant la lecture, il refuse, et ce refus est légitime et
    fréquent, pas une anomalie ;
11. il prend l'instantané, écrit, et journalise.

Le moment de la vérification est le cœur de la décision. Vérifier à la simulation
approuverait un plan calculé contre un état qui peut avoir bougé ; vérifier après
l'écriture ne vérifie rien ; vérifier sans re-simuler laisse la fenêtre pendant
laquelle l'humain lit. D'où l'étape 10.

Le matériau du défi est **préfixé champ par champ de sa longueur**, exactement la
discipline de `JournalEntry::digest` (`ks-core/src/journal.rs`, lignes 237 à 239) :
étiquette de domaine et de version, identifiant d'instance du broker, identifiant
de connexion, SID de l'appelant, nom du verbe, identifiant de plan, `plan_digest`,
date d'émission, date d'expiration, aléa de 32 octets tiré par `BCryptGenRandom`.
`plan_digest` couvre le nom du verbe, **tous** les paramètres, l'état relevé avant,
l'état calculé après, et l'identifiant de l'instantané pris. Sans les paramètres,
on signerait « désactiver un service » sans dire lequel.

**Ce format est figé avant la première signature**, pour la raison qui vaut pour le
journal : une signature déjà produite doit rester vérifiable. L'étiquette porte la
version ; une v2 est un format distinct, pas une migration silencieuse.

Une précision qui coûte cher si on la manque : `RequestSignAsync` reçoit les
**données** et les hache lui-même, tandis que `BCryptVerifySignature` reçoit le
**hachage**. Le broker calcule donc SHA-256 des octets du défi pour la
vérification. BLAKE3 sert au `plan_digest` à l'intérieur du matériau ; SHA-256 est
imposé par le schéma de signature. Deux fonctions, deux rôles, et les confondre
produit un refus permanent qu'on met une journée à diagnostiquer.

**Non mesuré, et il faut le lire avant de citer ce mécanisme** : le schéma de
signature exact. La documentation de `KeyCredentialManager` écrit « PKCS #1 RSA PSS
with SHA256 », formule qui nomme dans la même ligne deux schémas mutuellement
exclusifs. Le schéma se mesure **une fois en labo**, s'inscrit dans le code avec la
date et la version de build de Windows, et s'épingle par un vecteur enregistré que
l'intégration continue rejoue sans le fabriquer. Sont également non mesurés : la
disponibilité de `KeyCredentialManager` depuis un processus Win32 non empaqueté, et
la portée exacte de la clé pour une telle application.

**Une première rédaction ajoutait ici « la conception est écrite pour être juste
dans les deux cas, puisqu'elle n'a jamais supposé d'isolement par application ».
La phrase est vraie du mécanisme et fausse de la garantie**, et la distinction
décide de ce que SEC-08 tient contre A1. Le mécanisme est juste dans les deux cas :
rien dans le protocole ne suppose qu'une clé soit réservée à une application. Mais
si la clé enrôlée est utilisable par n'importe quel processus de la session, alors
A1 obtient son propre défi (§ 7 lui donne l'accès au tuyau, et la phase `simulate`
n'exige aucune présence), fait surgir sa propre invite au moment qu'il choisit, et
signe. La portée de la clé est donc le seul paramètre qui décide si SEC-08 résiste
à A1, et **elle n'est pas mesurée**. Tant qu'elle ne l'est pas, l'énoncé honnête
est celui du § 13 et de « Ce que ça ne garantit pas » : SEC-08 lie le geste au
diff, il ne prouve pas que le geste a été demandé par `ks`.

### 11. Le défi se consomme une fois, en mémoire seulement, et lié à la connexion

Table en mémoire du broker, **jamais sur disque** : un redémarrage invalide tous
les défis en cours, ce qui est le comportement voulu.

Cette volatilité est délibérée, et il faut la distinguer de son voisin immédiat :
la table des défis doit s'effacer au redémarrage, le compteur de limitation de débit
doit y survivre (§ 23). Les ranger ensemble « parce que c'est de l'état de session »
casserait l'un des deux.

Le retrait est atomique et **précède** la vérification ; une vérification en échec
ne réinsère pas. Un message mal formé consomme donc un défi et oblige à refaire le
geste : c'est un refus, jamais une écriture. Le défi est lié à la connexion, sans
quoi A1, qui porte le même SID, n'aurait qu'à rejouer sur la sienne la signature que
l'humain vient de produire pour la coque. L'échéance est courte, de l'ordre de deux
minutes ; **cette valeur est un jugement, pas une mesure**, jusqu'à ce que le labo
mesure le temps de lecture réel d'un diff de convergence. Un seul message de refus
couvre « déjà consommé », « expiré » et « jamais émis », le code technique vivant
dans le champ `detail`.

**La table est bornée, et son émission est tracée.** Une table en mémoire sans
plafond est un état que l'appelant fait grandir gratuitement : au plus **quatre
défis vivants**, autant que d'instances de tuyau, et l'émission d'un cinquième
refuse le plus ancien plutôt que d'allouer. Et **chaque émission de défi entre au
journal privilégié**, avec l'identifiant de connexion et le verbe visé, avant même
que l'humain voie quoi que ce soit. Cela ne ferme pas l'invite qu'A1 fait surgir,
rien ne le peut ici ; cela la rend **démontrable après coup**, ce qui est la seule
parade que ce document sait tenir contre lui, et c'est la même nature de parade
que pour le rendu falsifié du § 13. `ks journal` distingue donc les défis émis et
consommés des défis émis et jamais consommés : une invite que personne n'a
demandée laisse la seconde trace.

### 12. L'enrôlement exige une preuve de possession, la révocation est permanente, et le magasin vit là où vit le journal

L'enrôlement se fait côté client, par `RequestCreateAsync` puis
`RetrievePublicKey(BCryptPublicKey)`. Ce type de blob est choisi contre le défaut
`X509SubjectPublicKeyInfo` pour une raison précise : il s'importe directement par
`BCryptImportKeyPair`, **sans analyseur ASN.1 dans le composant élevé**. Retirer un
analyseur de format au broker vaut mieux que la commodité d'un format standard.

Le premier enrôlement est le moment où le broker n'a rien à quoi se raccrocher. Il
n'est accepté que si le magasin est vide, pendant une fenêtre à usage unique posée
par l'installeur ; il exige une signature sur un défi d'enrôlement, ce qui sert
aussi de recette du schéma de signature, au moment calme plutôt que le jour où
quelqu'un essaie d'isoler une machine ; le broker et le client impriment chacun
l'empreinte de la clé, et l'humain compare. C'est de la confiance à la première
rencontre avec vérification humaine, et il faut l'appeler par son nom.

Tout enrôlement ultérieur exige une signature d'une clé déjà enrôlée. La révocation
inscrit l'empreinte dans une liste de refus permanente, de sorte qu'un
ré-enrôlement de la même clé est rejeté. **Au moins deux clés sont enrôlées dès
l'installation.**

**Ce que ces deux clés couvrent, et ce qu'elles ne couvrent pas.** Une première
rédaction les justifiait par « une clé Windows Hello est perdue par une
réinitialisation de code confidentiel, une réinstallation ou un changement de
TPM » : or ces trois événements emportent **les deux** clés à la fois si elles
vivent dans le même conteneur Hello, même machine et même utilisateur. Deux clés
couvrent la perte d'une méthode d'authentification, ou d'un second utilisateur
enrôlé ; elles ne couvrent pas la perte du conteneur. La justification était
fausse, et elle laissait la panne qu'elle nommait sans issue : premier enrôlement
« seulement si le magasin est vide », enrôlements ultérieurs signés par une clé
déjà enrôlée, révocation permanente, aucun repli. Cinq verbes sur sept devenaient
**définitivement** indisponibles.

Le chemin de retour est donc écrit, et il tient parce qu'il exige ce qu'A1 n'a
pas. La fenêtre d'enrôlement à usage unique se **rouvre** par une opération
administrative sur le service : arrêt du service, dépôt d'un fichier de réouverture
dans le répertoire du journal privilégié, dont la DACL n'accorde la création qu'à
`SYSTEM` et `Administrateurs` (§ 18), redémarrage. Le broker consomme ce fichier,
rouvre la fenêtre pour un seul enrôlement, exige à nouveau la comparaison
d'empreinte par l'humain, et **journalise la réouverture** comme un événement de
premier plan, avec la liste des clés qui restaient enrôlées.

Ce n'est pas le repli que le § 13 refuse, et la différence est nette plutôt que
subtile : un repli est un chemin que l'appelant emprunte à la place de la preuve.
Celui-ci n'est pas un chemin de l'API, il n'a aucun destinataire côté tuyau, et il
suppose l'élévation sur la machine, c'est-à-dire un adversaire qui aurait déjà
tout ce que le broker protège. Ce que cela coûte, sans l'adoucir : **quiconque
détient l'administration de la machine peut enrôler sa propre clé**, ce qui était
déjà vrai puisqu'il peut remplacer le binaire (§ 14). La réouverture ne donne rien
à A2 ; elle rend la panne réparable pour l'utilisateur légitime.

**Le magasin des clés publiques vit là où vit le journal privilégié, hors de portée
de A1** (§ 18). Si A1 peut y écrire, tout ce paragraphe ne vaut rien : il enrôle sa
clé et signe ses propres défis. Cette phrase figure dans l'ADR au même niveau de
visibilité que la limite du § 7.

### 13. Sans preuve de présence disponible, les verbes coûteux sont refusés, et il n'y a pas de repli

Code de sortie distinct, refus journalisé, message à trois temps. Aucun `--force`,
aucune clé de configuration, aucune invite de remplacement.

Tout chemin de repli est celui que A1 empruntera systématiquement, donc la garantie
du système devient celle du repli. L'ADR-0006 a déjà tranché la forme symétrique de
cette question : « une confirmation qu'on ne peut pas évaluer n'est pas une
confirmation ». Ici : une preuve qu'on peut échanger contre une plus faible **est**
la plus faible.

Ce que cela coûte, sans l'adoucir : sur un poste sans Windows Hello configuré, cinq
verbes sur sept sont indisponibles. Keystone reste entièrement utilisable en
lecture. C'est cohérent avec un produit qui refuse déjà Windows 10 et les éditions
Famille. Le cas voisin, la perte de **toutes** les clés enrôlées sur un poste où
Hello fonctionne, a un chemin de retour, et il est écrit au § 12 : il passe par
l'administration du service, jamais par l'API.

Et ce que Hello ne prouve pas, à écrire sans adoucissement : il ne prouve pas un
**consentement éclairé**. La boîte de dialogue n'affiche pas les octets signés, qui
sont opaques à l'humain. La signature rend le geste **inséparable** du diff ; elle
ne fabrique aucune compréhension. Le diff lisible reste la condition (P6).

**Il ne prouve pas non plus quel processus a demandé, et une première rédaction se
rassurait ici d'une phrase fausse** : « ce qu'il ne peut pas, c'est signer un défi
qu'il n'a jamais reçu ». Rien n'empêche A1 d'en recevoir un. Le § 7 lui donne
l'accès au tuyau et le déclare explicitement ; le § 10 sert la phase `simulate`
sans aucune présence humaine. A1 ouvre donc sa propre connexion, demande une
simulation d'`Isolate` ou de `SetServiceStartup { EventLog, Disabled }`, reçoit un
défi authentique pour un plan authentique, et fait surgir l'invite Hello au moment
qu'il choisit. La re-simulation de l'étape 10 passe, le défi est bien lié à sa
connexion, la signature vaut bien pour son diff : aucune des trois barrières ne le
gêne, parce qu'aucune ne visait cela.

Ce qui reste entre lui et le résultat est **un humain qui refuse une invite qu'il
n'a pas demandée**, et rien d'autre, sauf si la clé enrôlée s'avère isolée par
application, ce que le § 10 déclare non mesuré. Tant que la mesure manque, la
formulation juste est : **SEC-08 ne résiste pas à A1**, il lie le geste au diff et
rend l'invite non sollicitée démontrable après coup (§ 11), ce qui est moins que
ce que le mot « présence » laisse entendre. La mesure de la portée de la clé est
une condition d'admission de ce paragraphe, pas un détail d'implémentation.

Et il ne survit pas à un client compromis dans son rendu : un `ks-ui` altéré affiche un texte
et fait signer l'empreinte d'un autre. Rien ici ne ferme cela, et rien ne le peut
sans un affichage de confiance que Windows n'offre pas. La parade est **forensique
et non préventive** : le broker journalise le texte exact du diff qu'il a émis, ce
qui rend l'écart démontrable après coup.

### 14. Le contrôle d'intégrité au démarrage vérifie ce que le broker charge, avec une racine épinglée, et ne se croit pas lui-même

L'ordre, avant le premier appel accepté :

| Rang | Contrôle | Échec |
|---|---|---|
| 1 | le jeton du processus est bien celui du service | refus de démarrer |
| 2 | DACL du répertoire d'installation : aucun principal hors `SYSTEM`, `Administrateurs`, `TrustedInstaller` en écriture, création, suppression ou propriété | lecture seule |
| 3 | DACL et propriétaire du répertoire du journal privilégié conformes, héritage coupé | **refus de démarrer** |
| 4 | signature Authenticode de chaque module et fichier que le broker chargera | lecture seule, et le module fautif n'est pas chargé |
| 5 | la chaîne obtenue est confrontée à une empreinte de clé publique **compilée dans le binaire** | lecture seule |
| 6 | création du tuyau, avec sa DACL et le drapeau de première instance | **refus de démarrer** (§ 6) |
| 7 | écriture de l'entrée d'amorçage, avant le premier accueil de connexion | refus de démarrer |
| 8 | signature du binaire lui-même, relue sur disque | consignée, **jamais traitée comme une preuve** |

Deux échecs seulement refusent le démarrage, et ce n'est pas arbitraire : un broker
qui ne peut pas garder de trace, ou qui ne possède pas le nom de sa propre
frontière, est plus dangereux à l'arrêt qu'en marche.

**Pourquoi la racine est épinglée plutôt que déléguée au magasin de la machine.**
Le § 6 du modèle de menace classe **premier** signal de valeur l'« ajout d'une
autorité de certification racine ». Déléguer la vérification de nos modules au
magasin racine ferait dépendre l'intégrité du produit précisément de ce que le
produit existe pour surveiller. Et le dépôt en donne lui-même la démonstration :
[la note de labo](../06-VM-DE-LABO.md) importe un certificat auto-signé dans
`LocalMachine\Root` et `LocalMachine\TrustedPublisher` en trois lignes. Une liste
blanche d'une clé ne se contourne pas comme une liste noire de certificats de test.
Est épinglée l'empreinte de la clé publique de la racine et de l'intermédiaire,
jamais le nom de sujet, jamais l'empreinte de la feuille seule ; deux clés sont
épinglées à tout instant, la courante et la suivante, faute de quoi le produit se
condamne le jour de l'expiration.

**Le piège, noir sur blanc :**

> L'auto-vérification d'un processus déjà chargé ne prouve rien. Quand ce contrôle
> s'exécute, le code qui l'exécute est déjà en mémoire. Si l'adversaire a remplacé
> le binaire sur le disque, c'est son code qui décide de faire le contrôle, de le
> réussir, ou de mentir sur son résultat. Un binaire substitué qui répond
> « intégrité vérifiée » est indiscernable d'un binaire authentique qui répond la
> même chose. Le rang 8 détecte une corruption accidentelle, une mise à jour
> incomplète, un fichier tronqué. Il ne détecte pas un adversaire, parce que c'est
> l'adversaire qui le passe.
>
> Ce qui protège réellement, c'est l'ACL du répertoire d'installation. Elle porte
> sur le futur, quand la vérification ne parle que du passé.

Le corollaire se mesure : `C:\Program Files` n'accorde à `BUILTIN\Utilisateurs` que
`(RX)` et réserve `(F)` à `TrustedInstaller`, `SYSTEM` et `Administrateurs` (M7).
C'est **cette ligne**, et non la signature, qui empêche A1 de remplacer le binaire.
Installer ailleurs, dans un répertoire dont A1 détient l'écriture, annulerait tout
ce paragraphe.

**Non vérifié, et assumé** : la révocation. La vérifier exige le réseau, que P5 et
SEC-12 refusent. Une clé de signature compromise puis révoquée en amont ne sera pas
remarquée localement ; la contrepartie est l'épinglage et SEC-11.

Une correction de prémisse, tant qu'à écrire ce paragraphe. Le `bcdedit /set
testsigning on` de la note de labo ne rend pas la chaîne de test acceptable pour le
broker : l'option gouverne la politique de signature **du mode noyau**, et
`ks-broker` est un service en mode utilisateur. Ce qui rend la build de labo
dangereuse en production, ce sont les deux importations de certificat vers les
magasins de la machine, et **l'épinglage les refuse par construction**. La phrase
de la note de labo qui enchaîne « refuse les modules non signés (SEC-07) » et
`testsigning` se corrige au même commit.

### 15. En mode lecture seule, seul `Scan` est servi, la simulation comprise dans le refus

| Verbe | Réponse en mode lecture seule |
|---|---|
| `Scan` | **servi**, la réponse portant la mention « intégrité non établie » |
| `TakeSnapshot` | refusé : un filet sans l'opération qu'il protège est du poids mort, et le prendre écrit sur le système |
| `RestoreSnapshot` | refusé, sans exception |
| `SetServiceStartup` | refusé, simulation comprise |
| `SetManagedSetting` | refusé, simulation comprise |
| `AddDefenderExclusion` | refusé, simulation comprise |
| `Isolate` | refusé : un broker suspect n'a pas à couper le réseau sur commande, et A1 obtiendrait sinon un déni de service en deux temps |

Le point le moins évident et le plus important : **la simulation est refusée elle
aussi**. Un diff produit par un broker dont l'intégrité n'est pas établie est un
diff dont rien ne garantit qu'il décrit la machine, et l'ADR-0021 a déjà tranché la
question sous une autre forme, « une simulation qui ment, ce qui est pire qu'une
absence de simulation ». Un utilisateur applique en confiance ce qu'il a vu simulé.

Le coût de ce refus est nul, et c'est vérifié : `ks diff` est calculé côté CLI, à
partir des collecteurs et du fichier d'état désiré, sans passer par le broker. Le
chemin de diagnostic reste entièrement ouvert pendant que le broker refuse.

**On ne quitte jamais ce mode par un appel d'API.** Ce n'est pas un booléen que
quelqu'un remet à zéro, c'est un typestate décidé une fois au démarrage :
`Broker<ReadOnly>` ne porte pas la méthode qui écrit. Il n'existe aucune fonction
qui le lève, aucun verbe qui la déclencherait, aucun réglage lu au vol. En sortir
suppose d'arrêter le service, de corriger la cause, de redémarrer : trois gestes qui
exigent les droits d'administration sur le service, donc pas A1.

### 16. Les codes de sortie sont distincts, et ils se documentent

`EXIT_PAS_ENCORE = 69` existe déjà (`crates/ks-cli/src/main.rs:273`) et n'est
documenté nulle part : `docs/08-CONVENTIONS.md` ne contient ni « code de sortie »,
ni « 69 ». C'est un comportement non documenté, donc un défaut, et le tableau
part au même commit.

**Deux espaces de codes, et ils ne se mélangent pas.** Une première rédaction les
rangeait dans un seul tableau, où le code 84 décrivait l'état d'un service alors
que ses voisins décrivent ce qu'un script appelant reçoit. Un script ne reçoit
jamais le code de sortie du broker ; il reçoit celui de `ks`.

Ce que `ks` rend à son appelant :

| Code | État |
|---|---|
| 0 | succès |
| 1 | erreur, et rien d'autre |
| 69 | pas encore implémenté, existant, conservé tel quel |
| 80 | broker en mode lecture seule : la demande a été comprise, refusée, journalisée, et rien n'a été écrit |
| 81 | politique gérée souveraine (P10) : jamais convergeable, réessayer est une erreur |
| 82 | présence humaine **refusée ou non donnée** : l'humain a annulé, ou le défi a expiré sans geste |
| 83 | limitation de débit atteinte : le seul de ces états où réessayer plus tard a un sens |
| 84 | le tuyau n'appartient pas à `SYSTEM`, ou le broker est injoignable : rien n'a été envoyé (§ 6) |
| 85 | présence humaine **indisponible** : Windows Hello n'est pas configuré, ou aucune clé n'est enrôlée. Le chemin de retour est au § 12 |
| 86 | **signature invalide** : une tentative, et non une annulation |

Les codes 82, 85 et 86 étaient un seul code dans la première rédaction. Les
confondre effaçait la distinction que ce document tient partout ailleurs : un
humain qui refuse n'est pas un poste sans Hello, et ni l'un ni l'autre n'est une
signature qui ne vérifie pas, laquelle est un événement de sécurité et se lit
comme tel dans le journal.

Ce que le service rend au gestionnaire de services, par
`ERROR_SERVICE_SPECIFIC_ERROR`, et qui n'apparaît dans aucun script :

| Valeur | État |
|---|---|
| 1 | journal privilégié inaccessible, DACL ou propriétaire non conformes (§ 14, rang 3) |
| 2 | nom de tuyau usurpé, refus de démarrer (§ 6) |
| 3 | jeton de processus inattendu (§ 14, rang 1) |

Au-delà de 69, `sysexits.h` n'a plus de vocabulaire pour ces états ; les plier dans
`EX_NOPERM` ou `EX_CONFIG` dirait quelque chose de faux.

### 17. Une fonction totale `contrat(&Verb) -> Contrat` vit en code de production, et son `match` n'a pas de bras `_`

C'est la réponse au premier constat de l'audit, et elle casse **la compilation**,
pas un test.

```rust
pub struct Contrat {
    pub simulation:   Simulation,
    pub annulation:   Annulation,
    pub presence:     Presence,
    pub idempotence:  Idempotence,
    pub lecture_seule: Reponse,
    pub debit:        Debit,
}
```

Trois propriétés font tout le travail. `Contrat` **n'implémente pas `Default`** et
tous ses champs sont obligatoires : une variante nouvelle ne s'ajoute pas avec une
valeur par défaut paresseuse. Chaque champ est une **énumération fermée**, dont les
variantes négatives portent une raison prise dans un ensemble fermé : `Presence`
n'est pas un booléen, c'est `NonRequise` ou `Requise { raison: CoutReel }`, et P6
exige qu'aucune invite ne puisse exister sans sa raison. Enfin la fonction vit en
**code de production** et non sous `#[cfg(test)]` : elle ne se supprime pas avec les
tests.

Le même mécanisme s'applique aux énumérations de paramètres, et c'est là qu'il
répond au second constat. Une première rédaction n'y nommait qu'une fonction
totale, `presence(&ManagedSetting)`, laissant `SnapshotSubject`, `ManagedService`,
`StartupType` et `SettingValue` sans aucune. **Éprouvé par falsification** hors
dépôt : ajouter trois variantes à `ManagedSetting` **et** une cible d'instantané à
`SnapshotSubject` compile et passe les treize tests. Ce que cela donnerait à A3 est
exactement la dette que l'ADR-0006 nomme comme la plus dangereuse : une cible
d'instantané qui fait exporter la ruche des comptes locaux par le broker, en
SYSTEM, sans qu'une barrière bronche. Une barrière qui couvre une des cinq
énumérations n'en couvre pas cinq.

Il y a donc **une fonction totale par énumération de paramètre**, toutes en code de
production, toutes à `match` exhaustif sans bras `_`, et toutes appelées depuis
`contrat` :

| Énumération | Fonction totale | Ce qu'elle oblige à décider |
|---|---|---|
| `ManagedSetting` | `contrat_du_reglage(&ManagedSetting) -> ContratReglage` | la clé cible, la classe de risque, la présence, ce qui part au journal, la réponse en lecture seule |
| `SnapshotSubject` | `contrat_du_sujet(&SnapshotSubject) -> ContratSujet` | ce qui est capturé, où l'artefact vit, ce que le filet ne couvre pas, l'ordre de grandeur écrit sur disque |
| `ManagedService` | `contrat_du_service(&ManagedService) -> ContratService` | ce que son arrêt signale **et** ce que son démarrage ouvre (§ 30) |
| `StartupType` | `effet_du_demarrage(&StartupType) -> EffetDemarrage` | ce que la valeur produit, et laquelle est un affaiblissement |
| `SettingValue` | `effet_de_la_valeur(&SettingValue) -> EffetValeur` | le sens de la valeur pour le réglage visé, et le cas de la valeur absente avant écriture |

Ajouter `DefenderTamperProtection`, `SecureBootPolicy` ou `ScriptExecutionPolicy` à
`ManagedSetting`, ou une cible à `SnapshotSubject`, **ne compile plus** tant que
personne n'a rempli la ligne correspondante. La barrière cesse de garder la seule
forme au moment où le contenu grandit.

**Une correction à la première rédaction, et elle n'est pas cosmétique.** Cette
liste d'exemples citait aussi `EntireRegistry` comme une variante « que la barrière
force à classer ». C'était une erreur de nature : `EntireRegistry` **est** le verbe
que le § 7 du modèle de menace interdit nommément, sous la forme d'une variante, et
aucune classification ne le rend admissible. L'ADR-0006 a déjà tranché la forme
symétrique de la question : « une confirmation qu'on ne peut pas évaluer n'est pas
une confirmation ». Une variante inadmissible ne se classe pas, elle se refuse ;
c'est l'objet du § 30.

Sa limite, à écrire pour ne pas la surestimer : le compilateur force le contributeur
à venir à l'endroit où les quatre questions sont écrites. **Il ne juge pas la
variante.** Aucun test grossier ne remplace la revue et l'ADR, et l'ADR-0007 le dit
déjà. C'est précisément parce qu'il ne juge pas que le § 30 existe.

### 18. Le journal privilégié vit hors de portée de A1, et sa DACL se relit au démarrage

Pas `%LOCALAPPDATA%`, et pas davantage `%ProgramData%` en se contentant de
l'héritage. La mesure M6 est le contre-exemple parfait à « les ACL du système
protègent » : `BUILTIN\Utilisateurs:(CI)(WD,AD,WEA,WA)` accorde à tout membre du
groupe la création de fichiers et de dossiers, et se propage aux conteneurs ; avec
`CREATEUR PROPRIETAIRE:(F)`, A1 peut **précréer** un fichier au nom prévisible et en
devenir propriétaire.

Trois clauses indissociables : `%ProgramData%\Keystone\journal\` avec **héritage
coupé** et propriétaire `SYSTEM` ; DACL explicite en droits individuels, `SYSTEM` en
écriture et création, `Administrateurs` en lecture, `Utilisateurs` en lecture seule
sur les fichiers, sans création ni suppression ; et la DACL **relue au démarrage**
et comparée à la valeur attendue, le service refusant de démarrer si elle a bougé.
Poser une ACL sans jamais la relire, c'est croire qu'elle est restée.

Le fichier s'ouvre en partage de lecture seule : même si la DACL venait à être
desserrée, aucun autre processus n'obtient une poignée d'écriture pendant que le
broker tient la sienne.

Contrepartie assumée : `Utilisateurs` **lit** le journal, ce qui permet à
`ks journal` de fonctionner sans nouveau verbe. A1 apprend donc ce que Keystone a
fait, ce qui a une valeur de reconnaissance. Le prix de l'alternative serait un
verbe de lecture du journal, c'est-à-dire une variante de plus dans l'énumération
qui est le contrat de sécurité du projet. On paie la reconnaissance plutôt que le
verbe, et cela rend **porteuse** la règle SEC-09 de rédaction à l'écriture : elle
cesse d'être de l'hygiène pour devenir la seule chose qui sépare ce journal d'une
fuite.

Le format d'écriture est la ligne JSON en ajout seul, par `serde_json`, déjà
présent : **zéro crate ajoutée**. SQLite coûterait six crates et une unité de
compilation C sur le composant élevé, pour une garantie que l'ajout de lignes rend
déjà, le broker étant l'unique écrivain. Pas de rotation avant la Phase 3 : faire
tourner un journal sans ancre externe, c'est effacer avec des étapes en plus.

### 19. Le format du journal se fige maintenant, et il porte les paramètres

La fenêtre est ouverte et elle se ferme à la Phase 3 : `ks journal --seal` refuse
aujourd'hui, faute d'ancre externe (`crates/ks-cli/src/main.rs`, aux environs de la
ligne 561), donc **aucun journal n'est jamais parti**. Après la première expédition,
tout ajout de champ fait recalculer une empreinte différente à un vérificateur
externe resté en arrière, qui déclare alors falsifiée une entrée intacte.

Six gestes, tous au même commit.

1. **Passer l'étiquette de domaine à `ks-journal-v2`.** Elle existe déjà dans le
   matériau (`ks-core/src/journal.rs:245`) et c'est exactement ce pour quoi elle a
   été mise.
2. **Ajouter les paramètres**, sous forme de couples ordonnés, en faisant entrer
   dans le matériau **le nombre de couples avant les couples**. Le fichier explique
   déjà pourquoi l'arité variable d'`Outcome` est sûre : son étiquette stable la
   précède, vient d'un ensemble fermé, et détermine l'arité. Rien ne joue ce rôle
   pour des paramètres de longueur libre, donc le compteur se met.
3. **Remplacer l'identité d'appelant** par une variante dérivée du jeton, portant le
   SID et le logon session, jamais lue dans le message. `Actor::Human(String)` reste,
   réservé au magasin de la CLI, et sa documentation dit qu'il n'est pas une
   prétention d'identité : sa valeur vient de l'environnement du processus
   (`ks-cli/src/main.rs:1071`), donc du parent, et la chaîne garantit que personne ne
   l'a modifiée **après coup**, jamais qu'elle est vraie.
4. **Remesurer les empreintes figées du test de non-régression et les recopier**,
   avec la raison écrite. `le_materiau_dune_entree_existante_na_pas_bouge`
   (`ks-core/src/journal.rs:358`) **doit échouer** à ce moment : c'est sa fonction, et
   c'est le seul instant où il sert.
5. **Vérification à deux régimes, et deux seulement.** La vérification essaie le
   matériau v2, puis le v1, et rend le régime constaté. Aucun champ de format n'est
   ajouté à l'entrée : il faudrait le faire entrer dans le matériau, ce qui est
   circulaire. **Ce sera la dernière tolérance de ce genre**, et la raison s'écrit
   dans le code.
6. **Poser la barrière qui manque** : la destructuration exhaustive de `Self` dans
   `digest()`, sans `..`. Ajouter un champ produit alors une erreur de compilation.
   Aujourd'hui, l'ajout d'un champ passe la suite au vert, ce qui rend la protection
   annoncée du format inexistante.

Et côté broker : les paramètres ne se remplissent jamais à la main. Une fonction
`parametres(&Verb)` à `match` exhaustif, au même endroit que `nom_du_verbe`, force à
décider ce qui part au journal pour chaque verbe. **Le journal enregistre le `Verb`
désérialisé et re-sérialisé sous forme canonique, jamais les octets reçus** : sans
cette règle, un appelant non privilégié choisirait le contenu d'un journal
privilégié, et SEC-09 deviendrait une fiction. Le défi et la signature n'y figurent
pas ; seules y figurent l'empreinte du défi consommé et l'empreinte de la clé qui a
signé.

**La rédaction porte sur le `diff` autant que sur les paramètres**, et c'est la
correction la plus lourde de ce paragraphe. Le journal est lisible par
`Utilisateurs` (§ 18) : la seule chose qui le sépare de l'oracle d'exfiltration que
le § 22 refuse comme verbe est la rédaction à l'écriture. Or le test annoncé portait
« sur les sept échantillons » de paramètres, et rien ne couvrait le champ `diff`,
qui est précisément celui qui portera l'état de BitLocker, l'état du TPM et le
contenu d'une exclusion. Un secret rédigé du paramètre et recopié dans le diff est
un secret sur disque, lisible par A1.

Ce qui est rédigé se liste, plutôt que de se deviner :

| Rédigé à l'écriture | Ce qui reste |
|---|---|
| clé de récupération BitLocker | sa présence, sa date, l'identifiant du protecteur |
| secret de propriétaire du TPM, valeurs de scellement | l'état du TPM, la liste des PCR concernés |
| contenu d'un export de registre ou de branche | son empreinte, sa taille, son emplacement |
| toute valeur qu'un collecteur marque comme secrète | son nom, son domaine, le fait qu'elle a changé |

La liste est **fermée par un type**, jamais par une fonction qui parcourt des
chaînes : un `ValeurJournalisable` ne se construit qu'à partir d'une valeur déjà
classée, et le classement vit dans le même `match` exhaustif que le reste. Une
valeur secrète non classée n'a pas de constructeur vers ce type.

### 20. Le journal privilégié fait foi, le magasin de la CLI ne fait foi que de lui-même

Le magasin de la CLI vit à portée d'écriture d'A1 (M6 par analogie, et
`ks-cli/src/magasin.rs:628` le reconnaît). Tout ce qu'il dit d'une opération
privilégiée est l'affirmation d'un processus non élevé au sujet d'un processus élevé.

**Les deux chaînes ne fusionnent jamais.** Chacune garde sa genèse, sa numérotation
et son ancrage. Les entrelacer suggérerait un ordre total que rien n'établit : deux
écrivains, deux horloges, aucune séquence commune. La règle vaut dans l'autre sens
et ferme un détour : **il n'existe aucun verbe d'écriture dans le journal.** Le
broker écrit le sien parce qu'il agit, jamais parce qu'un client le lui demande.

`ks journal` affiche donc deux sections, jamais mêlées, chacune précédée de ce
qu'elle prouve, avec sous chaque appelant la phrase du § 7 de la présente
décision. Et trois états distincts, jamais confondus : *absent*, *vide*, qui est un
signal et non un néant, et *illisible*, avec le code technique dans le champ
`detail`.

### 21. Aucun paramètre de verbe n'atteint jamais un interpréteur, et le validateur se dit ce qu'il ne borne pas

C'est un invariant, pas une intention : le broker n'appelle aucun interpréteur, ce
que garantit l'absence des verbes du § 7 du modèle de menace et la question
éliminatoire posée à chaque verbe ci-dessous.

Mais l'invariant ne dispense pas de la normalisation, et la mesure M5 le prouve.
Trois règles s'ajoutent à `ExclusionPath`, et elles sont dues **avant** la première
écriture de la Phase 2 :

1. **Le jeu de caractères se ferme.** Sont refusés les caractères de contrôle,
   l'octet nul, les caractères de contrôle bidirectionnel Unicode, et tout ce qui
   n'appartient pas au jeu que la sémantique Defender exige. Aujourd'hui aucun n'est
   refusé.
2. **Le chemin se normalise avant le diff, et le diff montre la forme normalisée.**
   « C:\progra~1 » est accepté quand « C:\Program Files » est refusé : le refus des
   répertoires système entiers est donc contournable par la forme courte. La
   normalisation Win32 précède le contrôle, faute de quoi le contrôle porte sur une
   chaîne et non sur un dossier.
3. **Le diff montre ce qui sera écrit, pas ce qui a été reçu.** C'est la règle que
   l'ADR-0006 a déjà posée pour les variables d'environnement, étendue à la forme
   courte et à la casse.

Ce que ces règles ne font pas, et qu'il faut écrire : elles bornent la sémantique
Defender et la normalisation Win32. Elles ne prétendent pas borner la syntaxe d'un
interpréteur, parce que **c'est l'absence d'interpréteur qui borne cela**, et que
faire reposer la garantie sur une liste de caractères serait la faire reposer sur la
liste qu'on aura oubliée.

**Et ces trois règles ont un mécanisme, faute de quoi elles n'étaient pas des
garanties.** La première rédaction les posait en prose, sans aucune ligne dans
« Ce qui doit casser », alors que ce tableau porte lui-même la phrase « une garantie
sans ligne dans ce tableau n'est pas une garantie ». Le mécanisme est un test de
non-régression sur les **dix cas de M5**, rejoués tels quels : les six premiers
doivent passer au refus, les trois derniers rester refusés, et un onzième cas,
`C:\PROGRA~1\keystone`, vérifie que la normalisation précède le contrôle et non
l'inverse. Le test est **éprouvé par falsification** : on retire la normalisation,
on vérifie que `C:\progra~1` redevient accepté, on restaure. Sans ce geste, la
règle 2 resterait une intention écrite au présent, ce que le dépôt interdit
ailleurs.

### 22. Les détours sont interdits nommément, parce qu'un détour anonyme revient

Aucun des verbes ci-dessous n'existe, et aucun n'est admissible sans ADR le
renversant explicitement :

| Détour | Pourquoi il est refusé |
|---|---|
| un verbe qui **lit un secret** et le rend à l'appelant | c'est un oracle d'exfiltration : le broker lit en SYSTEM ce que A1 ne peut pas lire. `Scan` rend un **état** (présent, absent, chiffré, activé), jamais une valeur secrète, et jamais une clé de récupération |
| la composition **prendre un instantané, altérer l'artefact, restaurer** | c'est l'écriture arbitraire par un chemin détourné. Parade dans « Les quatre questions », verbe `RestoreSnapshot` |
| un verbe de **configuration du broker lui-même** | il déplacerait toutes les décisions de ce document dans une donnée que l'appelant fournit |
| un verbe d'**écriture dans le journal** | § 20 |
| un verbe qui **quitte le mode lecture seule** | § 15. Le typestate rend la méthode inexistante, donc le verbe sans destinataire |
| un **export dont la cible est choisie par l'appelant** | c'est une écriture de fichier arbitraire en SYSTEM, sous un autre nom |
| une **écriture dans un répertoire chargé automatiquement** | démarrage, extensions de shell, chemin de recherche : c'est de l'exécution différée |
| `Scan { domain: Option<String> }` dont la chaîne libre atteindrait une requête WQL en SYSTEM | retiré, § 28 |
| une **variante de cible dont la sémantique donne ce qu'aucun verbe ne donne** | c'est le détour que l'appelant ne pilote pas, et que la première rédaction ne voyait pas. § 30 |
| une **réponse du broker qui porte une valeur secrète**, même si le journal la rédige | la rédaction du journal et la rédaction de la réponse sont deux surfaces distinctes. § 31 |

**Les huit premières lignes nomment des détours pilotés par l'appelant ; les deux
dernières nomment des détours internes au broker.** La distinction a été payée par
une revue : toutes les barrières de ce document ferment les voies où l'appelant
désigne une cible, et aucune ne regardait les deux endroits où c'est le **broker**
qui la choisit, la correspondance `ManagedSetting` vers une clé, et le contenu que
`Scan` rend. Le § 17 l'écrit lui-même, le `match` exhaustif « ne juge pas la
variante ». Les § 30 et § 31 sont ce jugement.

### 23. La limitation de débit survit au redémarrage, et elle se dérive du journal

Un compteur qui ne survit pas au redémarrage n'est pas une limite : il suffit de
faire redémarrer le service pour le remettre à zéro.

Le compteur ne vit donc pas en mémoire, et il ne vit pas non plus dans un fichier de
plus. Il **se dérive du journal privilégié**, en comptant les entrées des verbes
concernés dans la fenêtre. Trois bénéfices, dont deux non évidents : il n'y a rien à
synchroniser entre deux états ; il n'y a pas d'état qui puisse se réinitialiser en
silence, puisque le broker refuse déjà de démarrer si le journal est inaccessible
(§ 14, rang 3) ; et la limite est **auditable**, l'utilisateur pouvant recompter.

L'indexation se fait sur le SID dérivé du jeton, jamais sur une valeur choisie par
l'appelant. Coût assumé et non mesuré : la lecture de la queue du journal à chaque
verbe destructeur. Il est présumé négligeable ; « présumé » est écrit.

**Ce que ce compteur ne compte pas, et qui le compte à sa place.** Il porte sur les
verbes destructeurs, donc pas sur `Scan`, pas sur la phase `simulate`, pas sur
l'émission d'un défi : trois opérations qu'A1 peut demander en rafale sans jamais
franchir SEC-10. La rafale y coûte pourtant du travail au broker et fabrique des
invites. Ces trois-là sont donc bornées **par connexion et en mémoire**, au § 29, et
la différence de nature s'écrit plutôt que se déduit : une rafale de lectures ne
laisse rien de durable, donc sa borne peut se remettre à zéro au redémarrage ; une
rafale d'écritures laisse un système modifié, donc sa borne doit y survivre. Les
ranger ensemble casserait l'une des deux, comme le § 11 le dit déjà de la table des
défis.

### 24. P10 est appliqué par le broker, pas seulement par la CLI

Un conflit de politique gérée n'est jamais convergeable, et le traiter comme une
dérive déclenche une oscillation que Keystone perd à chaque cycle. Si ce refus ne
vivait que dans `ks-cli`, un client parlant directement au tuyau le contournerait.

Le broker refuse donc lui-même, avec le code de sortie 81, et journalise le refus.
La CLI garde son refus en amont : ce n'est pas une duplication, c'est un refus
précoce pour la lisibilité et un refus tardif pour la garantie.

### 25. SEC-12 l'emporte sur D16-03, et D16-03 se réécrit

SEC-12 dit « le broker n'écoute sur aucun port réseau ». D16-03
([cahier des charges](../01-CAHIER-DES-CHARGES.md), ligne 401) prévoit un exporteur
Prometheus, qui **est** un serveur HTTP en écoute. Les deux ne peuvent pas être
vrais.

Le présent document tranche dans le sens de SEC-12, et il le rend mécaniquement
vérifiable : `deny.toml` interdit nommément les crates capables d'ouvrir une socket
ou de servir du HTTP. D16-03 se réécrit donc en un **fichier de métriques au format
d'exposition Prometheus, écrit sur disque et cueilli par un agent tiers**, ou en une
expédition sortante initiée par la machine, cohérente avec ce que le cahier dit déjà
de la consultation à distance. Une exigence qui contredit une exigence de sécurité
se réécrit ; c'est la règle du § 3 du cahier des charges.

### 26. Chaque dépendance est chiffrée et justifiée contre l'alternative sans dépendance, et le compte est verrouillé

L'arbre de `ks-broker` compte **25 crates** (M1). La décision en ajoute **une**,
`windows-sys`, pour le tuyau, le descripteur de sécurité, le jeton du client et la
vérification de signature. `windows-link`, qu'elle tire, y est déjà. BLAKE3 y est
déjà. `serde_json` y est déjà.

| Voie | Coût mesuré | Décision |
|---|---|---|
| `windows-sys` | +1 | **retenue** |
| `tokio` minimal | +7 | tenue en réserve, non retenue : elle tire la machinerie des sockets dans un binaire qui promet de n'écouter aucun port, et son abstraction du tuyau passe de toute façon par un appel non sûr pour poser la DACL |
| `windows` (WinRT) dans le broker | +10 noms, plus un second `syn` majeur, soit onze entrées de verrou (M4) | refusée : la session 0 ne peut pas afficher d'invite, donc le gain n'existe pas |
| `tonic` seul, puis avec `prost` | +51, puis +55 (M2) | refusée, § 2 |
| `rusqlite` pour le journal | +6, plus une unité de compilation C | refusée, § 18 |
| `trybuild` pour prouver le typestate | **non mesuré** | non tranchée : la mesure qui manque est un `cargo tree` sur une branche jetable |

Et le compte se verrouille par deux mécanismes de nature différente, qu'il ne faut
pas confondre. Un test lit `cargo metadata` et compare l'ensemble des crates de
`ks-broker` à une liste versionnée : toute entrée nouvelle échoue avec le message
qui renvoie au § 7 du modèle de menace. Et `deny.toml` interdit nommément, dans sa
section `[bans] deny` restée vide jusqu'ici avec le commentaire qui appelait
exactement ce geste, les crates capables d'ouvrir une socket ou de parler HTTP. La
syntaxe qui permet d'accorder une dérogation nominative existe et a été vérifiée
(M8) :

```toml
deny = [
  { crate = "tokio", wrappers = ["ks-cli"] },
]
```

**Classification honnête : `deny.toml` casse la CI, pas la compilation**, et sa
portée est le graphe entier, pas un crate. Un contributeur qui désarme `deny.toml`
casse la revue, pas le build.

**Et ce chiffrage ne couvre que le broker, ce que la première rédaction ne disait
pas.** La signature Hello est faite par le **client** : c'est `ks-cli` qui doit
atteindre `KeyCredential`, donc WinRT. La conclusion « une crate ajoutée, sur
vingt-cinq » est donc vraie du composant privilégié et muette sur l'autre moitié
de la décision. Mesuré plutôt que supposé (M12) : `ks-cli` compte **103 crates**,
et `windows 0.62.2` **y est déjà**, tiré par `wmi 0.17.3` ; les dix crates que le
broker aurait dû ajouter y sont toutes présentes, `windows-future` et
`windows-threading` comprises. Le coût côté client est donc de **zéro nom de
crate**, et se réduit à deux choses qu'il faut tout de même écrire : `windows`
devient une dépendance **directe** de `ks-cli`, ce qui la rend visible et
révisable au lieu d'être un effet de bord de `wmi` ; et l'activation de la
fonctionnalité `Security_Credentials` élargit ce qui se compile de ce crate, donc
le temps de compilation, jamais la liste des crates. Le test « arbre figé et
compté » porte sur les deux crates, avec deux listes versionnées et deux comptes,
26 pour le broker et 103 pour la CLI.

### 27. Les blocs non sûrs sont nommés d'avance, et ils sont trois

`unsafe_code = "warn"` est déjà posé (`crates/ks-broker/Cargo.toml:35`) et **ne
devient jamais `allow`**. Toute liaison `windows-sys` étant un appel étranger, le
mot-clé apparaîtra ; il n'apparaîtra que dans trois modules, chacun portant son
commentaire `// SAFETY:` expliquant **pourquoi l'invariant tient** :

1. composer et poser le descripteur de sécurité du tuyau, et créer ses instances ;
2. lire le jeton du client, en usurpation, et revenir à soi ;
3. importer une clé publique, vérifier une signature, et tirer un aléa.

Tout autre bloc non sûr dans le broker passe par une ADR.

**Et il faut dire sous quelle forme, parce que la CI l'exige et que la première
rédaction ne le disait pas.** `.github/workflows/ci.yml` lance
`cargo clippy --locked --workspace --all-targets -- -D warnings` (ligne 116), broker
compris. La ligne 515 lance la même commande sous `working-directory: ui`, donc
sur le workspace séparé de l'ADR-0012, qui ne contient pas `ks-broker` : la citer
ici laissait croire à une double couverture qui n'existe pas : le premier bloc non sûr transforme donc
`unsafe_code = "warn"` en erreur, et la CI passe au rouge. Le fichier de CI décrit
lui-même ce piège (lignes 22 à 33) et la voie de moindre résistance qu'il ouvre,
un `#![allow(unsafe_code)]` global, que `rust.md` classe comme un changement
d'architecture.

La forme admise est donc **l'exception la plus étroite possible, jamais l'exception
globale** : `#[allow(unsafe_code)]` posé sur la **fonction** qui porte le bloc, avec
sur la même fonction son commentaire `// SAFETY:` expliquant pourquoi l'invariant
tient, et la référence à celle des trois familles ci-dessus à laquelle elle
appartient. Jamais au niveau du crate, jamais au niveau du module, jamais dans
`Cargo.toml`. Un test compte les occurrences de `allow(unsafe_code)` dans le source
du broker et **refuse au-delà de trois familles nommées**, ce qui rend le
quatrième bloc visible sans avoir à le chercher. La liste des trois familles
ci-dessus n'est donc pas une intention : c'est la valeur que ce test compare.

### 28. Le seul changement de surface est un retrait : `Scan` perd son paramètre

`Scan { domain: Option<String> }` (`ks-broker/src/main.rs:62`) devient `Scan`.

Trois raisons, et la troisième est la plus forte. C'est un **filtre d'affichage**,
comme la liste des exemptions le reconnaît elle-même (`main.rs`, ligne 821) : un
filtre d'affichage n'a aucune raison de franchir la frontière de privilège, le
client filtrant très bien ce qu'il a reçu. C'est une **chaîne libre sur un verbe qui
s'exécutera en SYSTEM**, donc le point d'entrée exact d'une injection dans une
requête d'inventaire. Et le retrait ne coûte rien aujourd'hui : `ks scan --domain`
résout le nom en `ks_core::Domain`, énumération fermée de onze variantes
(`crates/ks-core/src/item.rs:10`), et applique le filtre **côté client**
(`crates/ks-cli/src/main.rs:408` et `:425`). Le broker n'a jamais reçu cette valeur
et ne la recevra pas.

Deux conséquences à écrire, dont une défavorable.

La liste `CHAMPS_TEXTE_ADMIS` (`main.rs:817`) passe de deux entrées à **une**, la
dernière étant `AddDefenderExclusion.reason`, un motif destiné à un humain qui ne
désigne rien. La liste `TYPES_VALIDES_ADMIS` (`main.rs:865`), dont la clef est le triplet
`(variante, champ, type)`,
depuis le resserrement du 2026-08-23, n'est **pas touchée** : ce document n'y ajoute
rien et n'annule pas ce resserrement. Une entrée nouvelle dans l'une ou l'autre est
désormais un acte d'ADR.

Et le coût : `Scan` devient une variante **unitaire**, donc la seconde après
`Isolate` que `deny_unknown_fields` ne ferme pas (§ 3). Un
`{"verb":"scan","cmd":"calc.exe"}` sera accepté, le champ jeté, rien exécuté, et le
journal ne le verra jamais puisqu'il n'enregistre que la forme canonique. C'est un
coût réel, petit, et il vaut mieux que la chaîne libre qu'il remplace.

**« Petit » n'est vrai que sous la décision du § 3, et c'est à écrire ici parce que
c'est ici qu'on lit le coût.** Si la clef ne vivait que sur l'enveloppe, comme la
première rédaction du § 9 le décrivait, le coût ne serait pas deux variantes
unitaires : ce serait la surface entière, les sept verbes acceptant un champ inconnu
dans leur objet (M11). Le chiffre change d'un ordre de grandeur selon un détail de
rédaction qui n'était écrit nulle part ; c'est la raison pour laquelle le § 3 nomme
désormais les deux types.

### 29. L'hygiène des connexions est écrite, et la famine d'instances est nommée plutôt que niée

Quatre instances, `PIPE_WAIT`, aucun délai : A1, que la DACL admet, ouvre quatre
connexions, n'écrit rien, et Keystone devient injoignable. La source décrit
exactement ce qui est exploité : « the operations are not completed until there is
data to read, all data is written, or a client is connected. **Use of this mode can
mean waiting indefinitely in some situations for a client process to perform an
action** »
([CreateNamedPipeA](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea),
consulté le 2026-08-24). Le § 5 présentait `nMaxInstances = 4` comme une borne ;
c'est aussi l'outil, et la seconde moitié n'était pas écrite.

Quatre règles, et elles tiennent en peu de choses parce qu'elles ne prétendent pas
fermer ce qui ne se ferme pas :

1. **Un délai d'inactivité par connexion.** Une connexion qui n'a rien émis depuis
   dix secondes est fermée. La valeur est un **jugement**, pas une mesure.
2. **Un délai sur l'échange entier**, de la première trame à la dernière réponse,
   plus généreux que le précédent puisqu'il englobe l'attente d'un geste humain, et
   borné par l'échéance du défi (§ 11).
3. **Un plafond en mémoire, par connexion, sur ce que SEC-10 ne compte pas** :
   nombre de `Scan`, nombre de phases `simulate`, nombre de défis émis. Ce compteur
   se remet à zéro au redémarrage, et le § 23 dit pourquoi c'est correct ici et
   faux pour les verbes destructeurs.
4. **Toute connexion refusée, expirée ou fermée pour inactivité entre au journal
   privilégié.** Le § 6 du modèle de menace classe « silence de l'agent » au
   cinquième rang des signaux de valeur ; un déni de service qui ne laisse aucune
   ligne est un silence qu'on ne sait pas expliquer.

**Ce que cela ne ferme pas, et il faut le dire ici plutôt que dans une note.** A1
émet parfaitement des requêtes bien formées en boucle : il occupe alors les quatre
instances légitimement, et aucun délai ne l'en chasse. Contre un adversaire de la
même session, la disponibilité n'est pas défendable, et ce document ne prétendra
pas le contraire. Ce que les quatre règles donnent est différent et réel : le déni
devient **borné dans le temps**, **visible dans le journal**, et impossible à
obtenir en se taisant, ce qui est la forme la moins chère de l'attaque.

Relever `nMaxInstances` n'aiderait pas : cela multiplie le pool non paginé
consommé (§ 5) sans changer le fait qu'un adversaire de la session peut remplir ce
qu'on lui offre.

### 30. La cible que le broker choisit est jugée, pas seulement classée

Le § 17 rend impossible d'**ajouter** une variante de cible sans la classer. Il ne
dit rien de ce à quoi elle correspond, et il l'écrit lui-même. Or les deux endroits
où c'est le broker, et non l'appelant, qui choisit la cible sont exactement ceux
que rien ne gardait : la correspondance d'un `ManagedSetting` vers une clé de
registre, et la liste des services de `ManagedService`.

**La question éliminatoire se pose sur la clé que le broker écrira, jamais sur
l'absence de chaîne dans le message.** Un contributeur ajoute
`ManagedSetting::AllowInstallElevated`, le fait correspondre à
`…\Policies\Microsoft\Windows\Installer\AlwaysInstallElevated` sous les deux
ruches, et répond « oui » aux quatre questions sans mentir une seule fois : cela se
simule, cela s'annule, cela exige une présence, c'est idempotent, et aucun champ
libre ne traverse le tuyau. Le réglage, lui, vaut ceci : « This option is
equivalent to granting full administrative rights, which can pose a massive
security risk. Microsoft strongly discourages the use of this setting »
([AlwaysInstallElevated](https://learn.microsoft.com/en-us/windows/win32/msi/alwaysinstallelevated),
consulté le 2026-08-24). C'est `RunCommand` en SYSTEM, obtenu par un booléen
désigné, à travers toutes les barrières de ce document.

Deux listes de refus, donc, tenues dans le code du broker et confrontées à la table
de correspondance par un test qui casse la CI.

**Aucune variante de `ManagedSetting` ne correspond à une clé qui confère
l'exécution de code** : `…\Image File Execution Options\<exe>\Debugger`,
`…\Winlogon\Userinit` et `Shell`, `Services\<svc>\ImagePath`, `AppInit_DLLs`,
`…\CLSID\{…}\InprocServer32`, `…\Installer\AlwaysInstallElevated`. La liste est
celle que l'ADR-0006 avait déjà écrite pour justifier le retrait de
`SetRegistryValue`, plus la dernière ; elle est reprise ici parce qu'une liste qui
sert d'argument dans une ADR et qui n'existe nulle part dans le code n'empêche rien.

**Ni à une clé qui expose un secret ou retire une protection de secret** :
`…\SecurityProviders\WDigest\UseLogonCredential`, la désactivation de la protection
du processus LSA, celle de la sécurité fondée sur la virtualisation. Le cas WDigest
est nuancé par l'éditeur selon la version du système, et cette nuance ne change
rien : une variante dont l'effet dépend d'une version est une variante dont on ne
sait pas simuler l'effet, ce que P2 refuse déjà.

**Et `ManagedService` se juge sur les deux polarités.** Le source dit aujourd'hui
que la liste « ne contient que des services dont l'arrêt est un signal »
(`ks-broker/src/main.rs:138`). Le raisonnement ne couvre que la désactivation, et
il est muet sur une variante dont **l'activation** affaiblit la machine :
`RemoteRegistry`, `WinRM`, `sshd`, `RemoteAccess` placés en démarrage automatique
ouvrent, au prochain amorçage, un accès distant que rien dans ce document ne
surveille. Une variante n'est admissible que si **ni son arrêt ni son démarrage**
n'affaiblissent la posture ; aucun service dont l'activation ouvre un accès distant
ou un écouteur ne l'est. Le doc-commentaire du source se corrige au même commit.

Ce qu'aucun test ne fera : dire qu'une clé non listée est inoffensive. Les deux
listes attrapent ce qu'on a su nommer, et l'ADR-0007 a déjà écrit ce que vaut une
liste écrite à la main. Leur rôle est de rendre le raccourci bruyant et d'obliger
la question éliminatoire à porter sur le bon objet, pas de remplacer la revue.

### 31. Ce que `Scan` a le droit de rendre, et pourquoi ce n'est pas SEC-09

`Scan` est le verbe le plus exposé de l'énumération, et c'est contre-intuitif :
sans paramètre, sans présence humaine, servi même quand l'intégrité n'est pas
établie (§ 15), et exécuté en SYSTEM, donc voyant plus que la CLI (ADR-0021). Sa
réponse part directement à l'appelant, donc à A1.

Les quatre questions le déclarent inoffensif parce qu'il n'écrit rien. **La question
qui compte pour lui n'est pas ce qu'il écrit, c'est ce qu'il rend.** Une première
rédaction posait la bonne contrainte, « il rend un état et jamais une valeur
secrète », et la posait **en prose**, sans mécanisme et sans ligne dans « Ce qui
doit casser », c'est-à-dire selon la règle de ce document même : sans en faire une
garantie.

**Et elle la rangeait sous SEC-09, ce qui est une confusion de surface.** SEC-09
gouverne la **rédaction à l'écriture du journal**. La réponse de `Scan` ne touche
jamais le journal : une valeur parfaitement rédigée sur disque peut être rendue
intacte à l'appelant. Ce sont deux surfaces, et une seule était couverte.

La contrainte devient donc un type. Le broker ne rend jamais un `ItemValue` brut :
il rend une valeur qui a traversé un classement, et le type qui sort de ce
classement n'a **aucun constructeur** depuis une chaîne non classée. Concrètement,
`ItemValue::Text` et `ItemValue::List` (`crates/ks-core/src/item.rs`, lignes 229
et 231) sont exactement les deux variantes par lesquelles un secret peut sortir, et
ce sont elles que le classement doit traverser. Ce qui sort est l'état : présente,
absente, chiffrée, activée, illisible, avec sa raison. Ce qui ne sort jamais est la
valeur elle-même, pour la liste du § 19.

La ligne correspondante figure dans « Ce qui doit casser », et le test est
**éprouvé par falsification** : on fait rendre une clé de récupération par un
collecteur de laboratoire, on vérifie que la réponse la refuse, on restaure.

## Les quatre questions, pour les sept verbes déclarés

La question qui précède les quatre autres est posée à chacun : **ce verbe
permet-il, directement ou par détour, d'exécuter du code arbitraire ?**

### `Scan`

**Exécution arbitraire ?** Non, et c'est démontrable depuis le retrait : sans
paramètre, l'ensemble des lectures est celui que le broker code, et l'appelant n'en
désigne aucune.

1. **Se simuler ?** Sans objet : il ne change rien, donc il n'a pas d'état d'après.
   La question porte sur les verbes qui écrivent.
2. **S'annuler ?** Sans objet, même raison.
3. **Présence humaine ?** Non.
4. **Idempotent ?** Oui, sans effet.

**Contrainte propre**, et elle n'est pas mineure : le broker voit plus que la CLI
(ADR-0021). `Scan` rend donc des faits que A1 ne peut pas lire seul. Il rend un
**état** et jamais une valeur secrète : la présence d'une clé de récupération
BitLocker, jamais la clé ; l'état du TPM, jamais son contenu. Sans cette contrainte,
`Scan` serait l'oracle d'exfiltration que le § 22 refuse. **Cette contrainte est
portée par un type et non par la prose, et elle ne relève pas de SEC-09 : le § 31
dit pourquoi et comment.**

Et une correction : SEC-10 ne s'applique **pas** à `Scan`, contrairement à ce
qu'une première rédaction écrivait ici. La limitation de débit du § 23 se dérive du
journal privilégié en comptant les verbes destructeurs ; `Scan` n'en est pas un et
n'y figure pas. Ce qui borne la reconnaissance en rafale est le plafond par
connexion du § 29, qui est un mécanisme différent, plus faible, et qui se remet à
zéro au redémarrage.

### `TakeSnapshot`

**Exécution arbitraire ?** Non, `SnapshotSubject` étant une énumération fermée sans
champ. La forme précédente, `kind: String, target: String`, l'était (ADR-0006). Et
comme pour `SetManagedSetting`, la réponse ne vaut que pour les quatre variantes qui
existent : une cible d'instantané nouvelle décide de ce que le broker exporte en
SYSTEM, donc la question éliminatoire porte sur elle et non sur le message. C'est
ce que la fonction totale `contrat_du_sujet` du § 17 oblige à écrire, et ce que le
§ 30 juge.

1. **Se simuler ?** Oui : le diff annonce le mécanisme, l'emplacement, la taille
   attendue quand elle est mesurée, et ce que le filet ne couvrira pas.
2. **S'annuler ?** **Partiellement, et il faut le dire.** Keystone met en quarantaine
   ce qu'il détient (D4-04, quarantaine et jamais suppression). Un point de
   restauration système appartient à la plateforme : Keystone ne le défait pas, et
   l'ADR-0021 a déjà tranché qu'il ne purge que ce qu'il détient.
3. **Présence humaine ?** Non : fabriquer un filet ne défait rien.
4. **Idempotent ?** **Non**, et voici comment il est rendu sûr malgré tout. Deux
   prises produisent deux artefacts. Il ne détruit rien, la limitation de débit du
   § 23 borne la rafale, et la plateforme borne d'elle-même le point de restauration
   à un par vingt-quatre heures. Le danger d'une rafale est un déni d'espace, un
   export de branche large ayant été mesuré à 122 Mo (ADR-0021), pas une perte.
   **Ce verbe compte donc parmi les verbes que SEC-10 borne**, bien qu'il ne
   détruise rien : il écrit sur le système à la seule demande d'un appelant sans
   présence, et `WslDistributions` porte des ordres de grandeur sans commune mesure
   avec 122 Mo, `Debian` ayant été mesurée à 54,19 Go (ADR-0021). Le mot
   « destructeur » de SEC-10 se lit ici comme « qui consomme une ressource qu'on ne
   récupère pas en refusant ».

### `RestoreSnapshot`

**Exécution arbitraire ?** **Non, sous deux conditions sans lesquelles le verbe ne
s'implémente pas.** L'ADR-0006 l'écrit : restaurer, c'est appliquer en SYSTEM un
contenu que l'appelant désigne, et un instantané fabriqué donne l'écriture de
`Services\<svc>\ImagePath`, donc du code SYSTEM au démarrage. Les deux conditions
sont : l'identifiant se résout **par l'index du journal privilégié**, jamais par
concaténation à un chemin, ce que `SnapshotId` garantit déjà par son jeu de
caractères clos ; et l'artefact est **vérifié contre l'empreinte inscrite dans la
chaîne au moment de la prise**, l'artefact vivant lui-même hors de portée de A1, au
même endroit que le journal. Sans la seconde, la composition « prendre, altérer,
restaurer » du § 22 reste ouverte, et toutes les énumérations fermées de l'ADR-0006
sont contournées.

1. **Se simuler ?** Oui **pour les filets que Keystone fabrique**, dont il sait
   énumérer le contenu. Non pour un filet de plateforme, dont la portée n'est pas
   énumérable. Conséquence, et c'est un resserrement : ce verbe ne porte que le rang
   1 de l'ADR-0021. Le rang 2 se déclenche par l'outil de la plateforme, avec sa
   procédure documentée, parce que P2 exige un diff et qu'on ne sait pas le produire.
2. **S'annuler ?** Oui, par une prise de rang 1 avant la restauration. La récursion
   est finie : on ne restaure pas une restauration.
3. **Présence humaine ?** **Oui.** L'ADR-0006 l'appelle « le verbe le plus puissant
   de l'énumération ».
4. **Idempotent ?** Oui pour un export de branche : réécrire le même contenu est un
   non-événement, et le diff vide le montre.

### `SetServiceStartup`

Les quatre réponses de l'ADR-0006 sont **confirmées** et complétées.

**Exécution arbitraire ?** Non : `ManagedService` est sans champ, l'appelant ne peut
nommer aucun service hors des trois listés.

1. **Se simuler ?** Oui : lecture du type de démarrage, diff des deux valeurs.
2. **S'annuler ?** Oui, le type précédent étant une valeur conservée dans
   l'instantané.
3. **Présence humaine ?** Oui, systématiquement, la liste ne contenant que des
   services dont l'arrêt est un signal au sens du § 6 du modèle de menace, **et
   dont le démarrage n'ouvre rien** (§ 30). La seconde moitié de ce critère
   manquait : elle admettait `RemoteRegistry` ou `WinRM`, dont c'est l'activation
   qui est le geste d'attaque.
4. **Idempotent ?** Oui.

**Ce que l'ADR-0006 n'avait pas dit**, et que ce document ajoute : placer le journal
des événements en démarrage désactivé efface la piste d'audit. Le source du broker
le dit déjà de lui-même, à la déclaration de `ManagedService` : « Journal des
événements, sa perte efface la piste d'audit » (`main.rs:149`). Une première
rédaction attribuait ce fait au deuxième rang du § 6 du modèle de menace ; il n'y
figure pas, ce rang énumérant « temps réel, ASR, LSA, HVCI, BitLocker ». La
substance tenait, la citation non, et une ADR qui invoque une source qui ne dit
pas ce qu'on lui prête est précisément ce que ce dépôt traque. L'entrée de journal s'écrit donc **avant** l'écriture
système, jamais après ; sinon le verbe détruit la trace de lui-même.

### `SetManagedSetting`

Les quatre réponses de l'ADR-0006 sont confirmées, **avec une correction**.

**Exécution arbitraire ?** Non **pour les deux variantes qui existent**, et la
précision compte plus que la réponse. Le client désigne un réglage, le broker
détient la correspondance vers la clé ; la forme précédente,
`SetRegistryValue { hive, path, name, value }`, était le verbe que le § 7 du modèle
de menace interdit nommément. Mais la question éliminatoire porte alors sur la clé
que le broker écrira, et non sur ce que l'appelant transmet : une variante future
dont la clé confère l'exécution de code rendrait ce « non » faux sans qu'aucune
barrière de forme bronche. C'est l'objet du § 30, et c'est la condition à laquelle
cette réponse reste vraie quand la liste grandira.

1. **Se simuler ?** Oui, le broker connaissant la clé cible.
2. **S'annuler ?** Oui, par export préalable, en distinguant la valeur absente avant
   écriture, qui se restaure par une suppression et non par l'écriture d'un zéro.
3. **Présence humaine ?** L'ADR-0006 écrit « oui pour tout réglage classé sécurité,
   non pour les réglages de confort, dont la liste est fermée et revue ». **Cette
   phrase se corrige** : la classification n'est pas une liste, c'est une fonction
   totale à `match` exhaustif sur `ManagedSetting` (§ 17). Les deux variantes
   actuelles sont l'une et l'autre des réglages de sécurité, donc exigent la
   présence ; et une variante nouvelle **ne compile pas** tant qu'elle n'est pas
   classée.
4. **Idempotent ?** Oui, le diff vide montrant le non-événement.

### `AddDefenderExclusion`

**Exécution arbitraire ?** Non, et la nuance mérite d'être écrite plutôt que
tranchée d'un mot : une exclusion n'exécute rien, elle **désinhibe**. Elle crée un
angle mort dans lequel un code s'exécutera sans inspection. Ce n'est pas de
l'exécution arbitraire au sens du § 7 du modèle de menace, et c'est tout de même
la raison pour laquelle ce verbe porte trois contraintes obligatoires.

1. **Se simuler ?** Oui, **à condition** que le chemin montré soit la forme
   normalisée (§ 21). Le refus des variables d'environnement existe déjà pour cette
   raison, Defender tournant sous LocalSystem ; la forme courte 8.3 et la casse
   relèvent du même raisonnement et ne sont pas encore couvertes.
2. **S'annuler ?** Oui, en retirant l'exclusion, l'état précédent étant l'absence.
3. **Présence humaine ?** Oui : une exclusion est un angle mort volontaire.
4. **Idempotent ?** Oui.

La raison et l'expiration restent des paramètres obligatoires, pas des options
(D11-02), et l'horizon est borné à un an par `Expiry` : une dérogation sans plafond
est un angle mort permanent.

### `Isolate`

**Exécution arbitraire ?** Non, le verbe n'ayant aucun paramètre.

1. **Se simuler ?** Oui, et le diff doit dire la chose désagréable : après cette
   opération, ce poste n'est plus joignable par le réseau, et la remise en marche est
   un geste local. D14-06 parle de « coupure réseau hors canal d'administration » ;
   sur un poste personnel, ce canal n'existe pas, et la simulation ne doit pas
   laisser croire le contraire.
2. **S'annuler ?** **Oui, et c'est le point dur de ce verbe.** Deux exigences se
   contredisent si l'on n'écrit pas l'ordre : D14-06 inclut le scellement du journal,
   et SEC-04 l'expédie hors machine. Si le réseau est coupé d'abord, l'expédition ne
   part pas, et le verbe détruit la preuve qu'il prétend préserver. **L'ordre est
   donc : sceller, expédier, puis couper.** Le retour arrière nominal est la
   restauration du filet de rang 1 pris avant l'opération, ce qui n'ajoute aucun
   verbe ; le geste manuel documenté reste le secours. Et il faut écrire la
   conséquence : si la Phase 3 mesurait que ce retour ne tient pas, la sortie n'est
   **pas** d'ajouter un verbe de remise en marche, c'est de **retirer `Isolate`**
   jusqu'à ce qu'un retour existe, P3 faisant de la réversibilité un critère
   d'admission.
3. **Présence humaine ?** Oui, et c'est déjà écrit dans le source
   (`ks-broker/src/main.rs:112`).
4. **Idempotent ?** Oui : isoler une machine isolée est un non-événement.

**Non mesuré** : le mécanisme de coupure lui-même. Une règle de pare-feu bloquante
est réversible et journalisable ; désactiver un adaptateur l'est moins, et la
persistance au redémarrage change complètement la réponse à la question 2. Le choix
relève de la Phase 3 et n'est pas tranché ici.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| **Garder gRPC, comme l'ADR-0002 le décide** | ses énumérations sont ouvertes par spécification, donc incompatibles avec le fondement de SEC-02 ; elle embarque un serveur HTTP/2 dans un binaire qui promet de n'écouter aucun port ; elle coûte **51 crates** au binaire, **55** avec `prost`, plus un binaire externe hors verrou (M2 ; le chiffre de 49 qui figurait ici venait d'une position non rejouée, et il était faux) ; et son seul apport réel, la génération de clients dans d'autres langages, n'a aucun consommateur, l'interface dépendant de la CLI par chemin, en Rust |
| **Amender l'ADR-0002 plutôt que la remplacer** | deux de ses justifications sont fausses à la mesure et sa décision change. Une ADR ne se réécrit pas, elle se remplace, et l'ancienne reste lisible |
| **JSON-RPC 2.0 sur le tuyau** | zéro crate, enveloppe standard, et pourtant refusée : elle apporte une méthode nommée par une chaîne et des paramètres non typés au-dessus d'une énumération fermée, c'est-à-dire un risque pour une convention |
| **Trame préfixée de sa longueur** | écartée au profit de mieux : un préfixe de longueur est un entier choisi par l'appelant dont nous écririons nous-mêmes le plafond, l'arithmétique et l'allocation. Le noyau fait déjà ce travail |
| **Un codec binaire fermé** | aucun gain de sûreté sur `serde_json`, déjà présent et déjà lisible dans un journal et dans un rapport de laboratoire, pour un coût en crates sur le composant élevé |
| **COM ou DCOM** | argument de l'ADR-0002 conservé : surveiller les détournements COM avec COM est intenable |
| **HTTP local sur l'adresse de bouclage** | viole SEC-12 littéralement |
| **Un jeton de session partagé** | tout secret qu'un `ks` non élevé sait lire, A1 sait le lire. Aucune mesure ne peut réhabiliter ce mécanisme : l'argument découle de la prémisse du produit, pas d'une contingence de plateforme |
| **Identifier l'appelant par son PID, le chemin de son image, ou la signature de son binaire** | le PID nomme un processus et jamais le code qui s'y exécute ; le chemin est une chaîne que l'attaquant choisit, et reste exact quand il a écrit dans le vrai binaire ; la signature affirme quelque chose d'un fichier, pas d'une mémoire, et met un appel coûteux sur le chemin chaud du composant privilégié |
| **Exiger un client élevé, ou un compte de service distinct pour `ks`** | le premier viole SEC-01 et s'adosse à un mécanisme que l'éditeur ne traite pas comme une frontière ; le second suppose un secret d'ouverture de session que la session de l'utilisateur sait lire, donc que A1 sait lire |
| **Rapporter un booléen de présence depuis le client** | le résultat de la vérification va au processus appelant : A1, sous le même SID, remplace « vérifié » par une ligne |
| **Faire appeler Windows Hello par le broker** | impossible, et non pas insuffisant : un service vit en session 0 et ne peut pas interagir avec l'utilisateur |
| **Lancer un assistant dans la session de l'utilisateur** | déplace la question sans la traiter : l'assistant tourne sous le SID de A1, et le booléen qu'il rapporte reste un booléen. À moins qu'il ne signe, auquel cas on est revenu ici, avec un processus de plus à défendre |
| **Signer l'aléa seul, sans l'empreinte du diff** | c'est exactement « approuver un plan et en appliquer un autre » |
| **Signer l'empreinte du diff sans aléa** | rejouable indéfiniment |
| **Un aléa non lié à la connexion** | A1, même SID, présente sur sa propre connexion la signature que l'humain vient de produire |
| **Un mot de passe applicatif, un code confidentiel maison** | un secret de plus, dans la mémoire du client, donc à portée de A1, et c'est le chemin que A1 emprunterait toujours |
| **Une invite d'élévation à la place de Hello** | elle prouve un consentement à l'élévation, pas à l'opération. Le broker est déjà élevé |
| **Faire confiance au magasin racine de la machine pour vérifier nos modules** | le § 6 du modèle de menace fait de l'ajout d'une autorité racine le premier signal de valeur du produit. Trois lignes de la note de labo suffisent à peupler ce magasin |
| **Une liste noire des certificats de test** | un filtre se contourne et vieillit. Une liste blanche d'une clé, non |
| **Vérifier son propre binaire et en tirer une garantie** | le vérificateur est le vérifié. Le contrôle reste, sa conclusion change |
| **Refuser de démarrer plutôt que se dégrader sur un échec d'intégrité** | SEC-07 dit « démarre en mode lecture seule et alerte ». Refuser tout net supprime aussi la lecture qui permet de constater l'état, et supprime la trace |
| **Servir la simulation en mode lecture seule** | un diff non garanti induit une décision, et le coût du refus est mesuré nul, `ks diff` ne passant pas par le broker |
| **Un drapeau de contournement du mode lecture seule** | c'est le contournement écrit à l'avance, et il vivrait dans un fichier ou un environnement que A1 contrôle |
| **Garder un journal unique, celui de la CLI** | il vit à portée de A1. Un journal d'audit que A1 peut réécrire annule SEC-03 et SEC-04, qui sont les deux exigences que le journal existe pour servir |
| **`%ProgramData%` en héritant des ACL** | mesuré (M6) : le groupe des utilisateurs y a la création de fichiers et de dossiers, et le créateur propriétaire le contrôle total. Précréer un fichier suffit à en devenir propriétaire |
| **Un champ `target` enrichi qui concaténerait les paramètres** | réintroduit la collision par frontière déplacée que le préfixe de longueur interdit, et rend les paramètres inexploitables à la lecture |
| **Sérialiser le `Verb` entier dans le journal** | supposerait `Verb` hors de son binaire, donc la pente vers `#[non_exhaustive]` que le source documente comme désarmant la barrière SEC-02 |
| **SQLite pour le journal privilégié** | six crates et une unité de compilation C sur le composant élevé, pour une garantie que l'ajout de lignes rend déjà |
| **Attendre la Phase 2 pour ajouter les paramètres au journal** | c'est le raisonnement que l'ADR-0006 a déjà écarté : « en Phase 2, quelqu'un les implémentera tels qu'ils sont déclarés, parce que le contrat de l'API est réputé figé ». Ici c'est pire : après la Phase 3, le changement casse des vérificateurs qu'on ne contrôle plus |
| **Un compteur de débit en mémoire, ou dans un fichier dédié** | le premier se remet à zéro par un redémarrage du service ; le second est un second état à tenir cohérent avec le journal, pour aucun gain |
| **Typer `Scan { domain }` en énumération fermée plutôt que le retirer** | ce serait un changement de surface qui n'est pas un retrait, et il importerait `ks_core::Domain` dans le contrat du tuyau pour un filtre que le client applique déjà lui-même |
| **Exiger que la trame reçue soit identique à la re-sérialisation canonique** | fermerait les champs inconnus des variantes unitaires, et refusée : elle coupleraient la version de serde du client à celle du broker, si bien qu'une différence de formatage entre deux versions du produit deviendrait un refus de service sur notre propre client |
| **Un nom de tuyau par session, plutôt qu'une ACE de session ajoutée et retirée** | la voie existe et la source la mentionne, « If the application uses named pipes for IPC, the server can distinguish between multiple user processes by giving each pipe a unique name based on the session ID » ([Interactive Services](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services), consulté le 2026-08-24). Elle servirait plusieurs sessions à la fois, ce que la décision retenue ne fait pas. Écartée pour une raison de calendrier plutôt que de principe : le nom dépendant de la session, il ne peut se prendre qu'**après** l'ouverture de session, donc en concurrence avec A1, alors que le nom fixe se prend au démarrage de la machine (§ 6). On échange une régression fonctionnelle assumée contre une fenêtre de squattage rouverte à chaque ouverture de session. Si le multi-session devenait un besoin réel, c'est une ADR distincte, et elle devra traiter cette fenêtre |
| **Refuser `READ_CONTROL` au client, au titre de la reconnaissance** | c'était la première rédaction, et elle est renversée (§ 4) : la DACL est publiée dans ce document, donc rien n'est caché, tandis que le refus retirait au client la seule vérification bon marché de l'identité de son serveur |
| **Faire vérifier au client la DACL entière du tuyau, et non son seul propriétaire** | plus de surface pour rien : la DACL est une donnée que l'usurpateur compose comme il veut, alors que le propriétaire `SYSTEM` est hors de portée d'un processus non élevé. Comparer une chaîne SDDL ajouterait en prime un analyseur au chemin d'ouverture |
| **Une liste noire de caractères pour borner `ExclusionPath`** | c'est la liste qu'on aura oubliée (§ 21). La normalisation Win32 avant contrôle est ce qui ferme la forme courte, la casse et les alias, sans énumérer |

## Conséquences

### Ce que ça nous donne

Les sept verbes portent enfin leurs quatre réponses, et deux d'entre elles ne sont
pas celles qu'on attendait : `TakeSnapshot` n'est pas idempotent et le dit ;
`RestoreSnapshot` se resserre au seul rang de filet qu'il sait simuler.

L'élargissement futur de la surface casse la compilation, y compris quand il ne
s'agit que d'une variante de cible, et **pour les cinq énumérations de paramètres**
et non pour une seule. C'était le constat le plus inconfortable de l'audit : la
barrière gardait la forme au moment où seul le contenu allait grandir.

Et la variante ajoutée est désormais **jugée** en plus d'être classée, sur ce que
le broker en fera : la clé qu'il écrira, le service qu'il démarrera, la valeur
qu'il rendra (§ 30 et § 31). C'était l'angle mort de nature différente, celui où
l'appelant ne pilote rien et où toutes les barrières de forme regardaient ailleurs.

Le client sait à qui il parle. Ce n'était le cas d'aucune version précédente de
cette décision, ni de l'ADR-0002 : le nom d'un tuyau n'est pas une identité, et
`READ_CONTROL` plus un propriétaire `SYSTEM` en donnent une, pour le prix d'un
appel.

L'identité de l'appelant cesse d'être une promesse. Elle est dérivée du système,
donc infalsifiable, et elle est **inutile comme frontière**, ce qui est écrit dans
l'ADR, dans le code, et sous chaque ligne du journal que l'utilisateur lit.

La présence humaine cesse d'être un booléen. Le geste devient inséparable du diff
que le broker s'apprête à écrire, et l'ordre entre simuler et appliquer devient une
propriété du compilateur.

Le journal privilégié devient capable de porter SEC-03, et il le devient **avant**
la première expédition hors machine, c'est-à-dire au dernier moment où c'est
gratuit.

Une crate ajoutée au broker, sur vingt-cinq, et **zéro nom de crate ajouté à la
CLI**, `windows 0.62.2` y étant déjà par `wmi` (M12). Le coût de la sécurité de ce
document est mesurable, il porte sur les deux composants, et il est petit sur les
deux.

### Ce que ça nous coûte

**Cinq verbes sur sept exigent une présence humaine.** C'est beaucoup d'invites, et
c'est le principal risque d'usage de cette décision : un produit qui demande souvent
a déjà perdu, parce que l'humain finit par acquiescer sans lire. Le nombre d'invites
est donc lui-même un paramètre de sécurité, et c'est une raison de plus de tenir
SEC-10.

**Sur un poste sans Windows Hello configuré, cinq verbes sont indisponibles**, sans
repli d'aucune sorte.

**Le tuyau sert une seule session interactive à la fois.**

**Le format du journal se casse une fois, volontairement**, et un test de
non-régression doit passer au rouge pour cela. C'est un geste inconfortable, et
c'est le seul moment où il est possible.

**Trois familles d'appels non sûrs entrent dans le composant privilégié**, là où il
n'y en avait aucune.

**Une charge récurrente de laboratoire s'ajoute** : dix épreuves, à rejouer à
chaque changement de version du produit ou du système.

**Le client acquiert une dépendance directe à `windows`**, avec la fonctionnalité
`Security_Credentials`. Zéro nom de crate, mais une surface de compilation plus
large et une dépendance qui cesse d'être un effet de bord de `wmi` pour devenir un
choix. Elle est révisable, ce qui est le but.

**Un chemin de retour administratif existe pour l'enrôlement** (§ 12), et il faut
l'écrire au chapitre des coûts autant qu'à celui des acquis : qui détient
l'administration de la machine peut enrôler sa propre clé. Ce n'était pas moins
vrai avant, puisqu'il peut remplacer le binaire, mais c'est désormais une porte
nommée plutôt qu'une conséquence tacite.

**Trois codes de sortie de plus** (82 scindé en 82, 85 et 86), donc trois cas de
plus à traiter dans les scripts qui appellent `ks`.

**`ks-cli` perd la possibilité de dépendre un jour de `tokio`** sans une dérogation
nominative dans `deny.toml`, la portée de l'interdiction étant le graphe et non le
crate.

### Ce que ça corrige dans la documentation, et qui part au même commit

**[ADR-0002](0002-grpc-sur-named-pipe.md) passe en « Remplacé par ADR-0023 ».** Son
texte n'est pas modifié, sauf la ligne de statut. Trois de ses affirmations sont
reprises ici et corrigées : « un canal qui transporte du JSON libre invite à élargir
l'API par accident » repose sur une confusion, le contrat n'étant pas le format du
fil mais `Verb` ; « le contrat protobuf rend l'élargissement visible dans le diff »
est vrai mais insuffisant, et l'ouverture des énumérations proto3 le retourne ; et
« réinventer l'authentification que les ACL du pipe donnent gratuitement » est faux :
les ACL donnent un **contrôle d'accès par utilisateur et par session**, jamais une
authentification d'application.

**La décision d'ADR-0002 sur les VM et WSL2 est rouverte, sans être tranchée ici.**
`AF_HYPERV` n'a ni DACL, ni SID d'appelant, ni mode message. Elle relève d'une ADR
distincte, et jusque-là elle reste une intention et non un acquis. Une ligne le dit
dans l'ADR-0002 remplacée.

**[Cahier des charges](../01-CAHIER-DES-CHARGES.md), § 10.3, ligne 570.** « gRPC sur
named pipe, contrat versionné, jeton par session, verbes énumérés (SEC-02). Aucun
port TCP en écoute (SEC-12). » devient : « Tuyau nommé Windows en mode message,
trame JSON désérialisée en une énumération fermée de verbes (SEC-02), enveloppe
versionnée, identité de l'appelant dérivée du jeton du système. Aucun jeton partagé,
et aucun port en écoute (SEC-12). »

**Cahier des charges, schéma du § 3, ligne 122.** « named pipe + gRPC, jeton par
session » devient « tuyau nommé, mode message, identité dérivée du jeton ».

**[Architecture](../03-ARCHITECTURE.md), ligne 17.** « gRPC sur named pipe · jeton
par session » devient « tuyau nommé en mode message · identité dérivée du jeton ».

**[Feuille de route](../07-FEUILLE-DE-ROUTE.md), Phase 2, ligne 178.** « Broker :
service Windows, gRPC sur named pipe, jeton de session, ACL » devient : « Broker :
service Windows, tuyau nommé en mode message, DACL explicite en droits individuels
et logon SID, identité dérivée du jeton, aucun jeton partagé. »

**Cahier des charges, D16-03, ligne 401.** « Crochets sortants (webhooks) et
exporteur Prometheus, tous deux désactivés par défaut. » devient : « Crochets
sortants (webhooks) désactivés par défaut, et **fichier** de métriques au format
d'exposition Prometheus, écrit sur disque et cueilli par un agent tiers. Le broker
n'écoute sur aucun port (SEC-12). »

**[Note de labo](../06-VM-DE-LABO.md), ligne 54.** La phrase « `ks-broker` refuse
les modules non signés (exigence SEC-07) » suivie immédiatement de `testsigning`
laisse croire que la seconde sert la première. Elle ne la sert pas : `TESTSIGNING`
gouverne la politique de signature du mode noyau, et le broker est un service en
mode utilisateur. La note gagne la phrase : « Le certificat de développement ne
passe pas l'épinglage de `ks-broker` (ADR-0023, § 14) ; le mode de labo se
reconnaît, il ne se déguise pas. »

**[Conventions](../08-CONVENTIONS.md) gagne une section « Codes de sortie »**, avec
le tableau du § 16. `EXIT_PAS_ENCORE = 69` existe dans le code depuis la Phase 0 et
n'est documenté nulle part, ce qui est un défaut au sens de la règle « un
comportement non documenté est un défaut ».

**Le doc-commentaire de `nom_du_decideur`** (`crates/ks-cli/src/main.rs`, lignes 1063
à 1070), qui dit déjà « ce n'est **pas** une authentification », gagne la phrase qui
manque : la valeur est **choisie par l'appelant**, elle entre dans l'empreinte
chaînée, et la chaîne garantit donc que personne ne l'a modifiée après coup, jamais
qu'elle est vraie. Un lecteur pressé lit
« chaîné » et comprend « attesté ».

**L'ADR-0006 est précisée sur un point**, et cela figure ici plutôt que là-bas
puisqu'une ADR ne se réécrit pas : sa réponse « non pour les réglages de confort,
dont la liste est fermée et revue » devient une fonction totale à `match` exhaustif
(§ 17 et « Les quatre questions »).

**`deny.toml` gagne sa première interdiction nominative**, à l'endroit exact où son
commentaire l'annonçait.

**Le doc-commentaire de `ManagedService`** (`crates/ks-broker/src/main.rs`, aux
environs de la ligne 138) écrit que « la liste ne contient que des services dont
l'arrêt est un signal ». Le critère est juste et ne couvre qu'une polarité : il
gagne la seconde, « et dont le démarrage n'ouvre ni accès distant ni écouteur »
(§ 30). Le commentaire et la règle partent au même commit.

**Le doc-commentaire de `SnapshotSubject`** (même fichier, aux environs de la
ligne 211) dit ce que le sujet désigne ; il gagne la phrase qui manque, à savoir
que l'admissibilité d'une cible se juge sur ce que le broker exportera en SYSTEM,
et non sur l'absence de chaîne dans le message.

**`docs/08-CONVENTIONS.md` reçoit les deux tableaux du § 16**, celui des codes que
`ks` rend et celui des valeurs spécifiques rendues au gestionnaire de services, et
la phrase qui dit qu'ils ne se mélangent pas.

**`.github/workflows/ci.yml` reçoit, en commentaire, la forme admise de
`allow(unsafe_code)`** décidée au § 27, à côté du paragraphe qui décrit déjà le
piège de `-D warnings`. Le fichier explique la cause ; il lui manquait le geste.

### Ce que ça ferme

Tout accès distant direct au broker, comme l'ADR-0002 le fermait déjà, et de la même
manière, par conception.

L'idée qu'un composant puisse **déclarer** son identité, son besoin de présence, ou
sa phase d'exécution. Ces trois valeurs se dérivent, se calculent, ou se portent par
le typage ; aucune ne se lit dans un message.

L'idée qu'un secret partagé puisse séparer `ks` d'un adversaire qui partage sa
session. Cette porte est fermée par un argument, pas par une mesure, ce qui la ferme
plus solidement.

Et l'ajout d'une variante de cible sans revue. Il ne compile plus.

### Ce que ça ne garantit pas

**Rien contre A1 dans la même session.** C'est écrit trois fois dans ce document
parce que c'est la seule affirmation qu'une relecture pressée risque d'inverser. Le
transport n'est pas une frontière contre l'adversaire numéro un.

**Rien contre A2.** Qui obtient SYSTEM réécrit la DACL, prend le nom du tuyau, lit
la mémoire du broker, possède la table des défis et le magasin d'enrôlement, et
`verify_chain` répond « intacte ». Le modèle de menace l'assume déjà, et la parade
est l'ancrage externe, en Phase 3.

**Rien contre un rendu falsifié par le client.** Le diff vient du broker et le défi
porte son empreinte, mais le texte que l'humain lit est affiché par le client. La
parade est forensique, pas préventive.

**Rien contre la fatigue d'invite.**

**SEC-08 ne résiste pas à A1 tant que la portée de la clé enrôlée n'est pas
mesurée.** C'est l'attaque la plus grave qui reste ouverte, et elle se lit dans ce
document plutôt que sur une machine : A1 ouvre sa connexion, obtient une simulation
sans présence (§ 10), reçoit un défi authentique, fait surgir l'invite Hello au
moment de son choix, et il ne reste qu'un humain devant une boîte de dialogue qui
n'affiche ni le processus demandeur ni les octets signés. Ce qu'il faudrait pour la
fermer : un isolement de la clé par application, que l'interface `KeyCredential`
offre peut-être et dont **rien ici ne l'établit**. La mesure est due **avant** que
ce document passe de « Proposé » à « Accepté ». Ce qui existe en attendant est
partiel et il est écrit comme tel : le plafond de quatre défis vivants et
l'inscription au journal de chaque émission (§ 11), qui rendent l'invite non
sollicitée démontrable après coup.

**Rien contre un déni de service par occupation des instances.** Quatre connexions
bien formées suffisent, et A1 y a droit par la DACL. Le § 29 borne la durée et rend
le fait visible ; il ne rend pas Keystone joignable. Pour fermer cela, il faudrait
distinguer `ks` de A1 dans la même session, ce que le § 7 démontre impossible.

**Rien entre la re-simulation et l'écriture.** L'étape 10 du § 10 compare
`plan_digest` contre l'état courant, l'étape 11 prend l'instantané et écrit : rien
ne rend les deux indissociables, et la plateforme n'offre aucune transaction
couvrant à la fois le registre, un service et Defender. L'exploitation depuis A1 est
difficile, les cibles étant des énumérations fermées et l'artefact d'instantané
vivant hors de sa portée d'écriture, et la conséquence est bornée puisque
l'instantané précède l'écriture. C'est une fenêtre nommée, pas une fenêtre fermée.

**Rien si le magasin d'enrôlement devient inscriptible par A1.**

**Rien si le broker est installé dans un répertoire dont A1 détient l'écriture.**
Le typestate du mode lecture seule, l'épinglage de la racine et la vérification des
modules sont alors tous remplaçables par qui remplace le binaire.

**Le format d'échange n'est pas éprouvé.** Aucune ligne de ce transport n'existe.
Dix points sont spécifiés et non mesurés, et ils sont nommés : le logon SID sur le
poste de référence ; l'acceptation d'une étiquette d'intégrité par la création du
tuyau ; l'application d'un ACE ajouté après coup aux connexions ultérieures ; la
**non-réévaluation** d'une poignée déjà ouverte lorsque l'ACE de session est retiré
(§ 6) ; le comportement en mode message face à un message d'un octet de trop ;
`SetNamedPipeHandleState` depuis une poignée ouverte en droits individuels, dont la
résolution est décidée d'avance (§ 4) ; le comportement d'un client se connectant
en `SECURITY_ANONYMOUS` ou en `SECURITY_EFFECTIVE_ONLY` (§ 7) ; la lisibilité du
propriétaire du tuyau par `GetSecurityInfo` avec le masque écrit (§ 6) ; le schéma
de signature exact de Windows Hello ; et la **portée** de la clé pour un processus
Win32 non empaqueté, qui est celle dont dépend SEC-08 contre A1.

**Le coût de la relecture de DACL et de la lecture de la queue du journal n'est pas
mesuré.** Il est présumé négligeable ; « présumé » est écrit.

**Ce document ne dit rien du contenu des instantanés**, que l'ADR-0021 tranche, ni
de la forme du plan de convergence, ni de ce que la Phase 3 fera de l'ancrage
externe.

### Ce qui doit casser, et comment

Chaque garantie annoncée est classée par ce qui la fait tomber quand on l'enfreint.
Une garantie sans ligne dans ce tableau n'est pas une garantie.

**Casse la compilation**

| Garantie | Mécanisme |
|---|---|
| Tout verbe porte ses quatre réponses | `contrat(&Verb) -> Contrat`, `match` exhaustif sans bras `_`, en **code de production**, `Contrat` sans `Default` et à champs tous obligatoires |
| Toute variante de cible est classée, **pour les cinq énumérations** | `contrat_du_reglage(&ManagedSetting)`, `contrat_du_sujet(&SnapshotSubject)`, `contrat_du_service(&ManagedService)`, `effet_du_demarrage(&StartupType)`, `effet_de_la_valeur(&SettingValue)`, plus `en_lecture_seule(&Verb)` et `parametres(&Verb)`, mêmes règles. **Éprouvé par falsification** : on ajoute une variante à chacune des cinq, on vérifie que la compilation refuse cinq fois, on restaure |
| Une réponse du broker ne porte pas de valeur non classée | le type rendu n'a aucun constructeur depuis un `ItemValue::Text` ou `List` non passé par le classement (§ 31) |
| Chaque mode d'échec de la dérivation d'identité est traité | énumération d'échec incluant l'usurpation anonyme, `match` exhaustif (§ 7) |
| L'application vient après la simulation | `apply` n'existe que sur `Plan<Approved>`, seul constructible en consommant une `PresenceProof` |
| Une preuve n'approuve qu'un plan | `PresenceProof` : champ privé, aucun constructeur public hors du module de vérification, ni `Clone` ni `Copy` |
| Un broker en lecture seule ne peut pas écrire | la méthode n'existe pas sur `Broker<ReadOnly>` |
| L'identité ne se fabrique pas depuis un message | type à champs privés, unique constructeur public prenant la poignée du tuyau, origine en énumération à une seule variante filtrée par un `match` exhaustif |
| Un champ ajouté au journal ne passe pas inaperçu | destructuration exhaustive de `Self` dans `digest()`, sans `..` |
| Un module ne se charge pas avant sa vérification | le chargeur prend un type que seule la vérification construit, pas un chemin |
| Chaque mode d'échec du décodeur a son message utilisateur | `match` exhaustif sur l'énumération d'échec |

**Casse la CI, sans casser la compilation**

| Garantie | Mécanisme |
|---|---|
| Aucune crate capable d'ouvrir une socket dans le graphe | `deny.toml`, section `[bans] deny`, dérogations nominatives par `wrappers` (M8) |
| L'arbre du broker est figé et compté | un test lit `cargo metadata` et compare à une liste versionnée, 25 crates aujourd'hui, 26 après ce document |
| L'arbre de la CLI est figé et compté | même test, seconde liste versionnée, 103 crates aujourd'hui et après ce document, l'ajout de WinRT n'en ajoutant aucune (M12) |
| La MSRV se mesure | le job existant, qui est ce qui aurait révélé sans compiler qu'un client gRPC ne laissait aucune marge |
| Aucune lecture d'environnement dans le broker | contrôle textuel sur le source. Limite à écrire : une dépendance pourrait en lire une, et le contrôle ne le voit pas |
| Aucune identification par PID, chemin d'image ou signature d'appelant | contrôle textuel sur les noms d'API, plus la revue pour le raisonnement |
| `allow(unsafe_code)` reste par fonction, et n'excède pas trois familles | comptage des occurrences dans le source du broker, refus au-delà (§ 27). Sans cette ligne, `-D warnings` de `ci.yml:116` pousse vers l'exception globale |
| Aucune variante de `ManagedSetting` ne vise une clé d'exécution ou d'exposition de secret | la table de correspondance est confrontée aux deux listes de refus du § 30 |
| Aucune variante de `ManagedService` dont le démarrage ouvre un accès distant | même mécanisme, seconde polarité (§ 30) |

**Casse un test**

| Garantie | Mécanisme |
|---|---|
| Le masque du client ne porte aucun droit générique ni le droit de créer une instance | égalité sur la constante, et intersection nulle avec `0x0004` |
| Les huit paramètres de création sont ceux qui sont écrits ici | égalité champ par champ avec la constante unique par laquelle passe **chaque** instance (§ 5) |
| Le SDDL produit est exactement celui qui est écrit ici | fonction pure comparée caractère par caractère, sans horloge ni appel système dans le chemin asserté |
| Le SDDL ne contient aucun mnémonique agrégé | examen du texte produit, **éprouvé par falsification** : on injecte `FW`, on vérifie le rouge, on restaure |
| Le décodeur refuse la troncature, la trame vide, le verbe absent, le verbe inconnu | rejeu des cas mesurés, plus la trame d'un octet de trop |
| `deny_unknown_fields` agit aux **deux** niveaux | trois assertions : champ inconnu dans l'enveloppe refusé, champ inconnu dans une variante de structure refusé, champ inconnu dans une variante unitaire accepté et jeté. C'est le rejeu de M11, et il devient rouge si la clef migre d'un type à l'autre |
| Un champ inconnu sur une variante unitaire est accepté et jeté | test qui **documente le comportement réel** et deviendra rouge si serde change |
| `ExclusionPath` normalise avant de contrôler | rejeu des **dix cas de M5** plus `C:\PROGRA~1\keystone`, **éprouvé par falsification** : on retire la normalisation, on vérifie que `C:\progra~1` redevient accepté, on restaure (§ 21) |
| Deux clés au moins sont enrôlées, et une révocation ne se défait pas | l'enrôlement refuse de se conclure à une seule clé ; le ré-enrôlement d'une empreinte révoquée est refusé ; la réouverture administrative de la fenêtre exige le fichier que seul `SYSTEM` ou un administrateur peut créer (§ 12) |
| La limitation de débit survit au redémarrage | on écrit des entrées, on redémarre le broker, on vérifie que le compteur dérivé du journal n'a pas bougé (§ 23) |
| Un conflit de politique gérée est refusé **par le broker** | requête directe au tuyau, sans passer par `ks`, refus et code 81 (§ 24) |
| Le journal ne porte aucun secret, **`diff` compris** | test sur les sept échantillons **et** sur un diff fabriqué portant chacune des quatre catégories du § 19, avec garde-fou de non-vacuité |
| Une réponse de `Scan` ne porte aucune valeur secrète | **éprouvé par falsification** : un collecteur de laboratoire rend une clé de récupération, on vérifie que la réponse la refuse, on restaure (§ 31) |
| Le journal enregistre la forme canonique, jamais les octets reçus | on injecte une trame portant un champ inconnu, on vérifie qu'il n'apparaît pas dans l'entrée |
| Un défi ne sert qu'une fois | rejeu refusé |
| Une signature ne vaut que pour son diff | signature du plan A présentée pour le plan B, refusée. **Éprouvée par falsification** : on retire l'empreinte du matériau, on vérifie que la barrière passe au vert, on restaure |
| Le matériau du défi est préfixé de ses longueurs | deux découpages distincts produisent des matériaux distincts |
| Le geste ne se met jamais en cache | la constante est celle qui l'interdit |
| Le broker ne lit jamais le drapeau de présence porté par le plan | test dédié |
| Les codes de sortie sont deux à deux distincts, non nuls, différents de 1, et documentés | test qui lit aussi le fichier de conventions |
| Exactement deux régimes de vérification du journal, ni un ni trois | test avec garde-fou de non-vacuité |
| Le garde-fou de non-vacuité couvre tous les tests de rédaction | sans lui, un test qui ne parcourt rien est vert pour la pire des raisons |
| La vérification accepte le vecteur mesuré, et le refuse à un bit près | vecteur **produit une fois en labo** et versé au dépôt ; la CI le rejoue, elle ne le fabrique pas |

**Ne se vérifie que dans le laboratoire, et l'intégration continue ne peut pas le jouer**

C'est une limite reconnue plutôt que contournée : l'intégration continue tourne sur
Linux ou sur une machine Windows non représentative, et **`ks-broker` ne doit jamais
être installé comme service sur l'hôte**.

1. la création réelle du tuyau, sa DACL relue, et son étiquette d'intégrité ;
2. le refus de démarrer face à un squatteur, éprouvé en **prenant réellement le nom**
   avant le service, **et** la vérification du propriétaire côté client face à ce
   même squatteur : `ks` doit refuser et rendre 84 (§ 6) ;
3. l'usurpation du client et le SID rendu, comparé au SID attendu de la session ;
4. le refus d'un client se connectant en anonyme, et le comportement d'un client sans
   qualité de service de sécurité ;
5. **un binaire quelconque de la même session, sous le même utilisateur, ouvre le
   tuyau, est servi, et apparaît au journal sous le même SID que `ks`.** Si cette
   épreuve ne se produisait pas, la limite du § 7 serait de la rhétorique non
   éprouvée ;
6. le geste Hello réel, l'invite réelle, et le refus réel quand l'état bouge entre la
   simulation et l'écriture. **Et, dans la même épreuve, la portée de la clé** : un
   second binaire de la même session demande une simulation, obtient un défi, et
   tente `RequestSignAsync` avec la clé enrôlée. Ce que cette épreuve rend décide si
   SEC-08 résiste à A1, et c'est la mesure due avant le passage en « Accepté » ;
7. le desserrement réel de l'ACL du journal, et le refus de démarrer qui doit suivre ;
8. **la DACL réelle du répertoire d'installation**, `C:\Program Files\Keystone` et
   non son parent. M7 et M10 mesurent le parent et cinq répertoires voisins, où
   aucune ACE non héritée n'apparaît et où `CREATEUR PROPRIETAIRE` reste à héritage
   seul ; le rang 2 du § 14 contrôle au bon endroit, mais l'épreuve manquait ;
9. **la survie d'une connexion au retrait de l'ACE de session** : ouvrir, fermer la
   session, constater si le broker sert encore, et vérifier qu'il ferme lui-même la
   connexion comme le § 6 l'exige ;
10. **la famine d'instances** : quatre connexions muettes, puis mesure du délai au
    bout duquel `ks` est de nouveau servi, et vérification qu'une ligne de journal
    l'a enregistré (§ 29).

Chacune est horodatée et son compte rendu vit dans le dépôt. **Tant que ce compte
rendu n'existe pas, rien de ce document ne doit être présenté comme une garantie
tenue.**

Une remarque de méthode, mesurée : `cargo test -p ks-broker` n'exécute **aucun**
test de documentation, la cible étant un binaire seul. Un test de compilation refusée
écrit en documentation ne tournerait donc pas, et la preuve des typestates par
compilation refusée n'a **pas de véhicule automatisable en l'état**. Elle s'éprouve à
la main, dans le labo, et se consigne. La mesure qui manque pour trancher entre
« ajouter `trybuild` » et « éprouver à la main » est le nombre de crates que
`trybuild` ajouterait au graphe du broker.

**Ne se vérifie que par la revue**

Qu'une variante nouvelle soit *légitime*, et pas seulement *classée*. Le compilateur
force le contributeur à venir à l'endroit où les quatre questions sont écrites ; il
ne lit pas à sa place. Les deux listes de refus du § 30 attrapent ce qu'on a su
nommer, et l'ADR-0007 a déjà écrit ce que vaut une liste écrite à la main : elles
rendent le raccourci bruyant, elles ne rendent pas la revue superflue. Et qu'une
dépendance nouvelle soit justifiée contre l'alternative sans dépendance, chiffrée,
comme au § 26.
