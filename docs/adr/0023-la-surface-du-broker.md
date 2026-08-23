# ADR-0023 — La surface du broker, ce qui la franchit et ce qui la garde

- **Statut** : Proposé le 2026-08-24
- **Date** : 2026-08-24
- **Exigences concernées** : SEC-01, SEC-02, SEC-03, SEC-04, SEC-07, SEC-08, SEC-09, SEC-10, SEC-12, D11-02, D14-06, D16-03, P2, P3, P6, P10
- **Remplace** : [ADR-0002](0002-grpc-sur-named-pipe.md), qui passe en statut « Remplacé par ADR-0023 » et n'est pas supprimée
- **Complète** : [ADR-0004](0004-chainage-du-journal.md), [ADR-0006](0006-fermer-les-verbes-a-parametres-libres.md), [ADR-0007](0007-ce-qui-garde-lenumeration-des-verbes.md), [ADR-0021](0021-ce-quun-instantane-sait-defaire.md)
- **Ce document n'ajoute aucun verbe.** Il en retire un paramètre (§ 24), et c'est le seul changement de surface qu'il porte. Les quatre questions du modèle de menace y figurent pour les **sept** verbes déjà déclarés, cinq d'entre eux ne les ayant jamais reçues.

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
`SnapshotSubject`. Or l'ADR-0006 annonce explicitement que ces listes « se
peupleront en Phase 2 » : la surface qui va grandir est exactement celle que rien
ne garde.

**L'ADR-0002 décide un protocole dont deux justifications sont fausses à la
mesure**, et prescrit un « jeton par session » dont aucune position n'a su dire ce
qu'il apporterait.

**Le journal du broker n'existe pas, et sa forme actuelle ne peut pas porter
SEC-03.** `JournalEntry` compte neuf champs (`crates/ks-core/src/journal.rs`,
lignes 169 à 185) et **aucun** ne porte les paramètres, que SEC-03 exige
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

Les mesures ci-dessous ont été **rejouées** pour ce document, hors dépôt.
`git status --porcelain` ne montre aucune modification de fichier suivi du fait de
ces relevés.

| # | Mesure | Commande | Résultat |
|---|---|---|---|
| M1 | Arbre de `ks-broker` | `cargo tree -p ks-broker -e normal --target x86_64-pc-windows-msvc --prefix none`, dédoublonné | **25 crates**, dont `blake3 1.8.5` et `windows-link 0.2.1` déjà présentes |
| M2 | Coût de gRPC utilisable | relevé de la position « transport » | **+49** crates au binaire, **+71** à la compilation, plus `protoc`, binaire externe hors `Cargo.lock` |
| M3 | Coût de la vérification de signature | relevé de la position « présence humaine » | **+1** crate (`windows-sys`), `windows-link` étant déjà là |
| M4 | Coût de WinRT dans le broker | même position | **+11** crates et un second `syn` majeur dans le verrou |
| M5 | `ExclusionPath` face à une syntaxe d'interpréteur | copie du `TryFrom` du dépôt, hors dépôt, dix cas | voir ci-dessous |
| M6 | ACL de `C:\ProgramData` | `icacls C:\ProgramData` | `BUILTIN\Utilisateurs:(OI)(CI)(RX)` **et** `BUILTIN\Utilisateurs:(CI)(WD,AD,WEA,WA)`, `CREATEUR PROPRIETAIRE:(OI)(CI)(IO)(F)` |
| M7 | ACL de `C:\Program Files` | `icacls 'C:\Program Files'` | `BUILTIN\Utilisateurs:(RX)`, `NT SERVICE\TrustedInstaller:(F)`, aucune écriture pour `Utilisateurs` |
| M8 | `wrappers` de `cargo-deny` | `cargo deny check bans` sur une copie de `git archive HEAD` | syntaxe acceptée par cargo-deny 0.20.2, et `warning[unused-wrapper]` quand la dérogation ne sert pas |
| M9 | Suite du broker | `cargo test -p ks-broker` | 13 tests, tous verts |

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
la connexion : la borne de déni de service est obtenue sans une ligne de logique.

Ce que le mode message n'apporte pas, et qu'il ne faut pas lui prêter : un
écrivain hostile émet parfaitement un message court et complet. Le refus vient
alors du décodeur, pas du transport. Les deux couches sont nécessaires, aucune ne
suffit.

### 2. Le codage est du JSON désérialisé directement en `Verb`, et gRPC est écarté

`serde_json` est déjà dans l'arbre du broker (M1). gRPC coûterait 49 crates au
binaire élevé et 71 à la compilation (M2), plus `protoc`, binaire externe hors
`Cargo.lock`, donc hors `cargo deny check`, hors `cargo audit`, et hors du
verrouillage que le dépôt s'impose partout ailleurs.

La raison décisive n'est pourtant pas le compte. **proto3 emploie des énumérations
ouvertes** : un message vide se désérialise en la variante 0, et un champ inconnu
est ignoré en silence. Or toute la barrière SEC-02 repose sur la phrase que porte
`ManagedService` dans le source, « l'ensemble des cibles atteignables est fini,
énuméré, et lisible par un relecteur en un écran ». Un codec dont l'ensemble des
valeurs est ouvert par spécification ne peut pas porter ce contrat, et cela ne se
corrige pas en discipline de code.

Trois des crates que gRPC apporterait sont un serveur HTTP/2 complet et son
analyseur d'en-têtes, dans un binaire dont SEC-12 promet qu'il n'écoute sur aucun
port.

L'ADR-0007 a différé `syn` pour un coût en crates **nul**, au motif qu'il
« améliore l'auto-inspection du produit, pas le produit ». Si cet argument tient,
celui-ci tient a fortiori.

### 3. `deny_unknown_fields` est obligatoire sur le type de requête, et sa limite est écrite

La clef n'est pas dans `CLEFS_SERDE_ADMISES` (`ks-broker/src/main.rs:1011`, qui
admet `rename_all`, `tag` et `try_from`). Son inscription est un geste visible,
justifié ici : elle appartient à la même famille que `try_from`, c'est-à-dire aux
clefs qui **ajoutent** un contrôle au lieu d'en retirer un. Ce n'est pas un
assouplissement de la liste blanche, c'est son extension au seul motif qui la
justifie.

Sa limite, mesurée par la position « transport » : `deny_unknown_fields` ferme les
variantes **de structure**, pas les variantes **unitaires**.
`{"verb":"isolate","cmd":"calc.exe"}` est accepté, le champ étant jeté. Rien ne
s'exécute, et le journal ne le verra jamais (§ 19), mais le fait se documente par
un test qui décrit le comportement réel et deviendra rouge, donc visible, si serde
change.

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
O:SYG:SYD:P(A;;0x001f01ff;;;SY)(A;;0x00100183;;;{LOGON_SID})S:(ML;;NRNWNX;;;ME)
```

`{LOGON_SID}` est substitué à l'exécution par la forme rendue par
`ConvertSidToStringSidW`.

Le masque du client, `0x00100183`, se décompose ainsi, et **il est écrit en
hexadécimal exprès** :

| Droit | Valeur | Pourquoi |
|---|---|---|
| `FILE_READ_DATA` | `0x0001` | lire la réponse |
| `FILE_WRITE_DATA` | `0x0002` | émettre la trame |
| `FILE_READ_ATTRIBUTES` | `0x0080` | `GetNamedPipeInfo` |
| `FILE_WRITE_ATTRIBUTES` | `0x0100` | `SetNamedPipeHandleState`, pour que le client passe **sa propre** poignée en mode message |
| `SYNCHRONIZE` | `0x00100000` | attendre sur la poignée |

Et ce qui est refusé compte davantage :

| Refusé | Valeur | Conséquence s'il était accordé |
|---|---|---|
| `FILE_APPEND_DATA`, identique à `FILE_CREATE_PIPE_INSTANCE` | `0x0004` | **squattage du nom** |
| `READ_CONTROL` | `0x00020000` | lecture de la DACL, donc reconnaissance |
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

`S:(ML;;NRNWNX;;;ME)` pose une étiquette d'intégrité obligatoire au niveau moyen.
Elle ne protège **pas** contre A1, qui s'exécute au même niveau. Elle ferme le cas
que l'ADR-0002 citait elle-même, « un script dans un onglet de navigateur » : un
moteur de rendu s'exécute en intégrité basse ou en conteneur d'application, et le
noyau lui refuse la connexion avant toute lecture.

Le logon SID est le seul mécanisme disponible pour séparer deux sessions du même
utilisateur, et c'est la page ci-dessus qui l'indique : « To prevent remote users
or users on a different terminal services session from accessing a named pipe, use
the logon SID on the DACL for the pipe. »

**Non mesuré, et nommé pour l'épreuve de labo** : la présence et la lisibilité du
logon SID sur le poste de référence, le fait qu'un descripteur portant une
étiquette d'intégrité soit accepté par `CreateNamedPipeW` sans privilège
supplémentaire, et le fait que `SetNamedPipeHandleState` fonctionne depuis une
poignée ouverte avec ces droits individuels plutôt qu'avec `GENERIC_READ`, que la
documentation cite. Si ce dernier point se révélait faux, la résolution est
d'élargir le masque **des seuls bits mesurés nécessaires**, documentés un par un,
jamais de basculer sur un mnémonique agrégé.

### 5. Les drapeaux de création sont écrits, et aucun n'est laissé au défaut

```
dwOpenMode    = PIPE_ACCESS_DUPLEX (0x00000003)
              | FILE_FLAG_FIRST_PIPE_INSTANCE (0x00080000)   [première instance seulement]
dwPipeMode    = PIPE_TYPE_MESSAGE (0x00000004)
              | PIPE_READMODE_MESSAGE (0x00000002)
              | PIPE_WAIT (0x00000000)
              | PIPE_REJECT_REMOTE_CLIENTS (0x00000008)
nMaxInstances = 4, jamais PIPE_UNLIMITED_INSTANCES
```

`PIPE_ACCEPT_REMOTE_CLIENTS` vaut `0x00000000` : **accepter les clients distants
est ce qu'on obtient en n'écrivant rien**. C'est le genre de défaut qu'une revue
ne voit pas, puisqu'il n'y a rien à lire.

Trois pièges à écrire, parce qu'ils se paient à l'exécution et non à la revue.
`FILE_FLAG_FIRST_PIPE_INSTANCE` vaut `0x00080000`, la même valeur numérique que
`WRITE_OWNER` dans un autre paramètre. Le drapeau se pose sur la **première**
instance seulement, les suivantes échouant sinon. Et **chaque instance porte son
propre descripteur de sécurité** : la DACL se fournit à chaque appel, pas au
premier.

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

Le mode lecture seule de SEC-07 **ne s'applique pas ici** : il couvre l'échec d'un
contrôle d'intégrité, où le broker sait encore qui il est. Ici il ne sait plus s'il
est joignable, et démarrer en lecture seule laisserait l'usurpateur répondre aux
lectures, c'est-à-dire fournir à A1 un inventaire du poste sous l'apparence de
Keystone.

La fenêtre de squattage se ferme par l'ordre : le service prend le nom **au
démarrage de la machine**, avec une DACL ne portant que l'ACE `SY`, puis ajoute
l'ACE de session à l'ouverture de session interactive et le retire à sa fermeture.
Coût assumé : le tuyau sert **une seule session interactive à la fois**. La bascule
rapide d'utilisateur n'est pas servie ; sur un poste personnel, périmètre déclaré
du produit, ce n'est pas une régression.

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
| limiter le débit par client | un compteur persistant indexé sur le SID (§ 22) |
| corréler un dialogue au journal | l'identifiant de connexion ci-dessus |

### 9. L'enveloppe porte une phase, jamais un verbe de plus, et l'ordre est un typestate

La requête est `{ "protocol": "…", "phase": "…", "verb": { "verb": "…", … } }`, avec
`deny_unknown_fields` sur l'enveloppe. `phase` est une énumération **fermée à deux
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
cannot directly interact with a user as of Windows Vista ».

Le protocole, dans l'ordre exact :

1. le client ouvre la connexion ; le broker dérive le SID à l'acceptation (§ 7) ;
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
la portée exacte de la clé pour une telle application. La conception est écrite pour
être juste dans les deux cas, puisqu'elle n'a jamais supposé d'isolement par
application.

### 11. Le défi se consomme une fois, en mémoire seulement, et lié à la connexion

Table en mémoire du broker, **jamais sur disque** : un redémarrage invalide tous
les défis en cours, ce qui est le comportement voulu.

Cette volatilité est délibérée, et il faut la distinguer de son voisin immédiat :
la table des défis doit s'effacer au redémarrage, le compteur de limitation de débit
doit y survivre (§ 22). Les ranger ensemble « parce que c'est de l'état de session »
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
l'installation** : une clé Windows Hello est perdue par une réinitialisation de code
confidentiel, une réinstallation ou un changement de TPM, et rien ne la restaure.

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
Famille.

Et ce que Hello ne prouve pas, à écrire sans adoucissement : il ne prouve pas un
**consentement éclairé**. La boîte de dialogue n'affiche pas les octets signés, qui
sont opaques à l'humain. La signature rend le geste **inséparable** du diff ; elle
ne fabrique aucune compréhension. Le diff lisible reste la condition (P6). Il ne
prouve pas non plus quel processus a demandé : A1 fait surgir sa propre invite quand
il veut ; ce qu'il ne peut pas, c'est signer un défi qu'il n'a jamais reçu. Et il ne
survit pas à un client compromis dans son rendu : un `ks-ui` altéré affiche un texte
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

| Code | État |
|---|---|
| 0 | succès |
| 1 | erreur, et rien d'autre |
| 69 | pas encore implémenté, existant, conservé tel quel |
| 80 | broker en mode lecture seule : la demande a été comprise, refusée, journalisée, et rien n'a été écrit |
| 81 | politique gérée souveraine (P10) : jamais convergeable, réessayer est une erreur |
| 82 | présence humaine non obtenue : refus, absence, ou signature invalide |
| 83 | limitation de débit atteinte : le seul des quatre où réessayer plus tard a un sens |
| 84 | canal usurpé, refus de démarrer (§ 6) |

Au-delà de 69, `sysexits.h` n'a plus de vocabulaire pour ces états ; les plier dans
`EX_NOPERM` ou `EX_CONFIG` dirait quelque chose de faux. Côté service, l'échec de
démarrage se rapporte au gestionnaire de services par
`ERROR_SERVICE_SPECIFIC_ERROR`, avec trois valeurs spécifiques : journal
inaccessible, nom de tuyau usurpé, jeton de processus inattendu.

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
répond au second constat. `presence(&ManagedSetting)`, `parametres(&Verb)` et
`en_lecture_seule(&Verb)` sont des `match` exhaustifs. Ajouter
`DefenderTamperProtection`, `SecureBootPolicy`, `ScriptExecutionPolicy` ou
`EntireRegistry` à `ManagedSetting` **ne compile plus** tant que personne n'a dit si
ce réglage exige une présence, ce qu'il journalise, et ce qu'il répond en lecture
seule. La barrière cesse de garder la seule forme au moment où le contenu grandit.

Sa limite, à écrire pour ne pas la surestimer : le compilateur force le contributeur
à venir à l'endroit où les quatre questions sont écrites. **Il ne juge pas la
variante.** Aucun test grossier ne remplace la revue et l'ADR, et l'ADR-0007 le dit
déjà.

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
qu'elle prouve, avec sous chaque appelant la phrase du § 7. Et trois états
distincts, jamais confondus : *absent*, *vide*, qui est un signal et non un néant, et
*illisible*, avec le code technique dans le champ `detail`.

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

### 22. Les détours sont interdits nommément, parce qu'un détour anonyme revient

Aucun des verbes ci-dessous n'existe, et aucun n'est admissible sans ADR le
renversant explicitement :

| Détour | Pourquoi il est refusé |
|---|---|
| un verbe qui **lit un secret** et le rend à l'appelant | c'est un oracle d'exfiltration : le broker lit en SYSTEM ce que A1 ne peut pas lire. `Scan` rend un **état** (présent, absent, chiffré, activé), jamais une valeur secrète, et jamais une clé de récupération |
| la composition **prendre un instantané, altérer l'artefact, restaurer** | c'est l'écriture arbitraire par un chemin détourné. Parade au § 27, verbe `RestoreSnapshot` |
| un verbe de **configuration du broker lui-même** | il déplacerait toutes les décisions de ce document dans une donnée que l'appelant fournit |
| un verbe d'**écriture dans le journal** | § 20 |
| un verbe qui **quitte le mode lecture seule** | § 15. Le typestate rend la méthode inexistante, donc le verbe sans destinataire |
| un **export dont la cible est choisie par l'appelant** | c'est une écriture de fichier arbitraire en SYSTEM, sous un autre nom |
| une **écriture dans un répertoire chargé automatiquement** | démarrage, extensions de shell, chemin de recherche : c'est de l'exécution différée |
| `Scan { domain: Option<String> }` dont la chaîne libre atteindrait une requête WQL en SYSTEM | retiré, § 24 |

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
| `windows` (WinRT) dans le broker | +11, plus un second `syn` majeur | refusée : la session 0 ne peut pas afficher d'invite, donc le gain n'existe pas |
| `tonic` et `prost` | +49 au binaire, +71 à la compilation, plus `protoc` | refusée, § 2 |
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

### 27. Les blocs non sûrs sont nommés d'avance, et ils sont trois

`unsafe_code = "warn"` est déjà posé (`crates/ks-broker/Cargo.toml:35`) et **ne
devient jamais `allow`**. Toute liaison `windows-sys` étant un appel étranger, le
mot-clé apparaîtra ; il n'apparaîtra que dans trois modules, chacun portant son
commentaire `// SAFETY:` expliquant **pourquoi l'invariant tient** :

1. composer et poser le descripteur de sécurité du tuyau, et créer ses instances ;
2. lire le jeton du client, en usurpation, et revenir à soi ;
3. importer une clé publique, vérifier une signature, et tirer un aléa.

Tout autre bloc non sûr dans le broker passe par une ADR.

### 28. Le seul changement de surface est un retrait : `Scan` perd son paramètre

`Scan { domain: Option<String> }` (`ks-broker/src/main.rs:62`) devient `Scan`.

Trois raisons, et la troisième est la plus forte. C'est un **filtre d'affichage**,
comme la liste des exemptions le reconnaît elle-même (`main.rs`, ligne 820) : un
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
désigne rien. La liste `TYPES_VALIDES_ADMIS` (`main.rs:865`), indexée par quadruplet
depuis le resserrement du 2026-08-23, n'est **pas touchée** : ce document n'y ajoute
rien et n'annule pas ce resserrement. Une entrée nouvelle dans l'une ou l'autre est
désormais un acte d'ADR.

Et le coût : `Scan` devient une variante **unitaire**, donc la seconde après
`Isolate` que `deny_unknown_fields` ne ferme pas (§ 3). Un
`{"verb":"scan","cmd":"calc.exe"}` sera accepté, le champ jeté, rien exécuté, et le
journal ne le verra jamais puisqu'il n'enregistre que la forme canonique. C'est un
coût réel, petit, et il vaut mieux que la chaîne libre qu'il remplace.

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
`Scan` serait l'oracle d'exfiltration que le § 22 refuse. SEC-10 s'applique malgré
tout, contre la reconnaissance en rafale.

### `TakeSnapshot`

**Exécution arbitraire ?** Non, `SnapshotSubject` étant une énumération fermée sans
champ. La forme précédente, `kind: String, target: String`, l'était (ADR-0006).

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
   services dont l'arrêt est un signal au sens du § 6 du modèle de menace.
4. **Idempotent ?** Oui.

**Ce que l'ADR-0006 n'avait pas dit**, et que ce document ajoute : placer le journal
des événements en démarrage désactivé efface la piste d'audit, ce que le § 6 classe
au deuxième rang des signaux. L'entrée de journal s'écrit donc **avant** l'écriture
système, jamais après ; sinon le verbe détruit la trace de lui-même.

### `SetManagedSetting`

Les quatre réponses de l'ADR-0006 sont confirmées, **avec une correction**.

**Exécution arbitraire ?** Non : le client désigne un réglage, le broker détient la
correspondance vers la clé. La forme précédente, `SetRegistryValue { hive, path,
name, value }`, était le verbe que le § 7 interdit nommément.

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
l'exécution arbitraire au sens du § 7, et c'est tout de même la raison pour laquelle
ce verbe porte trois contraintes obligatoires.

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
| **Garder gRPC, comme l'ADR-0002 le décide** | ses énumérations sont ouvertes par spécification, donc incompatibles avec le fondement de SEC-02 ; elle embarque un serveur HTTP/2 dans un binaire qui promet de n'écouter aucun port ; elle coûte 49 crates au binaire et 71 à la compilation, plus un binaire externe hors verrou ; et son seul apport réel, la génération de clients dans d'autres langages, n'a aucun consommateur, l'interface dépendant de la CLI par chemin, en Rust |
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

## Conséquences

### Ce que ça nous donne

Les sept verbes portent enfin leurs quatre réponses, et deux d'entre elles ne sont
pas celles qu'on attendait : `TakeSnapshot` n'est pas idempotent et le dit ;
`RestoreSnapshot` se resserre au seul rang de filet qu'il sait simuler.

L'élargissement futur de la surface casse la compilation, y compris quand il ne
s'agit que d'une variante de cible. C'était le constat le plus inconfortable de
l'audit : la barrière gardait la forme au moment où seul le contenu allait grandir.

L'identité de l'appelant cesse d'être une promesse. Elle est dérivée du système,
donc infalsifiable, et elle est **inutile comme frontière**, ce qui est écrit dans
l'ADR, dans le code, et sous chaque ligne du journal que l'utilisateur lit.

La présence humaine cesse d'être un booléen. Le geste devient inséparable du diff
que le broker s'apprête à écrire, et l'ordre entre simuler et appliquer devient une
propriété du compilateur.

Le journal privilégié devient capable de porter SEC-03, et il le devient **avant**
la première expédition hors machine, c'est-à-dire au dernier moment où c'est
gratuit.

Une crate ajoutée, sur vingt-cinq. Le coût de la sécurité de ce document est
mesurable et il est petit.

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

**Une charge récurrente de laboratoire s'ajoute** : sept épreuves, à rejouer à
chaque changement de version du produit ou du système.

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

**`crates/ks-cli/src/main.rs:1063`**, dont le commentaire dit déjà « ce n'est pas une
authentification », gagne la phrase qui manque : la valeur est **choisie par
l'appelant**, elle entre dans l'empreinte chaînée, et la chaîne garantit donc que
personne ne l'a modifiée après coup, jamais qu'elle est vraie. Un lecteur pressé lit
« chaîné » et comprend « attesté ».

**L'ADR-0006 est précisée sur un point**, et cela figure ici plutôt que là-bas
puisqu'une ADR ne se réécrit pas : sa réponse « non pour les réglages de confort,
dont la liste est fermée et revue » devient une fonction totale à `match` exhaustif
(§ 17 et « Les quatre questions »).

**`deny.toml` gagne sa première interdiction nominative**, à l'endroit exact où son
commentaire l'annonçait.

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

**Rien si le magasin d'enrôlement devient inscriptible par A1.**

**Rien si le broker est installé dans un répertoire dont A1 détient l'écriture.**
Le typestate du mode lecture seule, l'épinglage de la racine et la vérification des
modules sont alors tous remplaçables par qui remplace le binaire.

**Le format d'échange n'est pas éprouvé.** Aucune ligne de ce transport n'existe.
Sept points sont spécifiés et non mesurés, et ils sont nommés : le logon SID sur le
poste de référence ; l'acceptation d'une étiquette d'intégrité par la création du
tuyau ; l'application d'un ACE ajouté après coup aux connexions ultérieures ; le
comportement en mode message face à un message d'un octet de trop ;
`SetNamedPipeHandleState` depuis une poignée ouverte en droits individuels ; le
schéma de signature exact de Windows Hello ; et la disponibilité de l'interface de
clé depuis un processus Win32 non empaqueté.

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
| Toute variante de cible est classée | `presence(&ManagedSetting)`, `en_lecture_seule(&Verb)`, `parametres(&Verb)`, mêmes règles |
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
| La MSRV se mesure | le job existant, qui est ce qui aurait révélé sans compiler qu'un client gRPC ne laissait aucune marge |
| Aucune lecture d'environnement dans le broker | contrôle textuel sur le source. Limite à écrire : une dépendance pourrait en lire une, et le contrôle ne le voit pas |
| Aucune identification par PID, chemin d'image ou signature d'appelant | contrôle textuel sur les noms d'API, plus la revue pour le raisonnement |

**Casse un test**

| Garantie | Mécanisme |
|---|---|
| Le masque du client ne porte aucun droit générique ni le droit de créer une instance | égalité sur la constante, et intersection nulle avec `0x0004` |
| Le SDDL produit est exactement celui qui est écrit ici | fonction pure comparée caractère par caractère, sans horloge ni appel système dans le chemin asserté |
| Le SDDL ne contient aucun mnémonique agrégé | examen du texte produit, **éprouvé par falsification** : on injecte `FW`, on vérifie le rouge, on restaure |
| Le décodeur refuse la troncature, la trame vide, le verbe absent, le verbe inconnu | rejeu des cas mesurés, plus la trame d'un octet de trop |
| Un champ inconnu sur une variante unitaire est accepté et jeté | test qui **documente le comportement réel** et deviendra rouge si serde change |
| Le journal enregistre la forme canonique, jamais les octets reçus | on injecte une trame portant un champ inconnu, on vérifie qu'il n'apparaît pas dans l'entrée |
| Un défi ne sert qu'une fois | rejeu refusé |
| Une signature ne vaut que pour son diff | signature du plan A présentée pour le plan B, refusée. **Éprouvée par falsification** : on retire l'empreinte du matériau, on vérifie que la barrière passe au vert, on restaure |
| Le matériau du défi est préfixé de ses longueurs | deux découpages distincts produisent des matériaux distincts |
| Le geste ne se met jamais en cache | la constante est celle qui l'interdit |
| Le broker ne lit jamais le drapeau de présence porté par le plan | test dédié |
| Les codes de sortie sont deux à deux distincts, non nuls, différents de 1, et documentés | test qui lit aussi le fichier de conventions |
| Exactement deux régimes de vérification du journal, ni un ni trois | test avec garde-fou de non-vacuité |
| Aucun paramètre journalisé ne porte de secret | test sur les sept échantillons, avec garde-fou de non-vacuité, sans quoi il est vert pour la pire des raisons |
| La vérification accepte le vecteur mesuré, et le refuse à un bit près | vecteur **produit une fois en labo** et versé au dépôt ; la CI le rejoue, elle ne le fabrique pas |

**Ne se vérifie que dans le laboratoire, et l'intégration continue ne peut pas le jouer**

C'est une limite reconnue plutôt que contournée : l'intégration continue tourne sur
Linux ou sur une machine Windows non représentative, et **`ks-broker` ne doit jamais
être installé comme service sur l'hôte**.

1. la création réelle du tuyau, sa DACL relue, et son étiquette d'intégrité ;
2. le refus de démarrer face à un squatteur, éprouvé en **prenant réellement le nom**
   avant le service ;
3. l'usurpation du client et le SID rendu, comparé au SID attendu de la session ;
4. le refus d'un client se connectant en anonyme, et le comportement d'un client sans
   qualité de service de sécurité ;
5. **un binaire quelconque de la même session, sous le même utilisateur, ouvre le
   tuyau, est servi, et apparaît au journal sous le même SID que `ks`.** Si cette
   épreuve ne se produisait pas, la limite du § 7 serait de la rhétorique non
   éprouvée ;
6. le geste Hello réel, l'invite réelle, et le refus réel quand l'état bouge entre la
   simulation et l'écriture ;
7. le desserrement réel de l'ACL du journal, et le refus de démarrer qui doit suivre.

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
ne lit pas à sa place. Et qu'une dépendance nouvelle soit justifiée contre
l'alternative sans dépendance, chiffrée, comme au § 26.
