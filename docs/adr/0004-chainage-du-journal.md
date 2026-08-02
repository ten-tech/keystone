# ADR-0004 — Chaîner le journal par BLAKE3 non clé, et écrire ce que ça ne garantit pas

- **Statut** : Accepté
- **Date** : 2026-08-02
- **Exigences concernées** : SEC-03, SEC-04, D1-09, P6

## Contexte

Le journal de Keystone chaîne ses entrées : chacune porte l'empreinte de la
précédente, si bien qu'on ne peut ni en retirer une ni en réécrire une sans
casser la suite. C'est le mécanisme sur lequel reposent SEC-03 (intégrité du
journal) et SEC-04 (détection d'une altération).

L'empreinte est aujourd'hui calculée par **FNV-1a**, une fonction de hachage non
cryptographique. Le code l'annonce comme un bouchon, ce qui était la seule
manière honnête de la laisser en place, et la feuille de route prévoit son
remplacement en Phase 0.5.

Trois faits cadrent la décision.

**FNV-1a n'oppose aucune résistance.** Fabriquer une seconde entrée de même
empreinte se fait à la demande, en quelques millisecondes. La chaîne actuelle ne
détecte donc même pas une altération volontaire élémentaire.

**Un hachage non clé, même cryptographique, ne protège pas d'un attaquant
privilégié.** L'ancrage de la chaîne est public, l'algorithme est public, et
l'attaquant qui obtient SYSTEM peut recalculer la totalité des empreintes après
avoir réécrit ce qu'il voulait. La chaîne redevient parfaitement cohérente, et
rien ne le signale. C'est l'adversaire A2 du modèle de menace, et c'est
précisément celui contre lequel un journal d'audit est censé servir.

**La parade existe mais n'est pas atteignable aujourd'hui.** Une empreinte clée
par une clé scellée dans le TPM résiste à cet attaquant : il peut effacer le
journal, il ne peut pas le réécrire de façon cohérente. Sceller une clé exige
d'appeler le TPM depuis un composant privilégié, donc `ks-broker`, qui n'existe
pas encore et n'existera qu'en Phase 2.

## Décision

On remplace FNV-1a par **BLAKE3 non clé** en Phase 0.5, et l'ADR écrit
explicitement ce que la chaîne **ne** garantit **pas**. Le chaînage clé et le
scellement TPM feront l'objet d'une ADR distincte, en Phase 2, quand le broker
existera.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| **Garder FNV-1a** jusqu'à la Phase 2 | La feuille de route promet le remplacement en 0.5, et une promesse tenue plus tard est une promesse fausse aujourd'hui. Surtout, une fonction qui se collisionne à la demande ne détecte rien du tout : la chaîne serait décorative, ce que le principe P6 refuse. |
| **BLAKE3 clé, clé protégée par DPAPI** | Un attaquant SYSTEM déchiffre DPAPI. La garantie resterait donc la même contre A2, au prix d'une gestion de clé — création, perte, rotation — à écrire dès maintenant. Pire : le mot « clé » ferait croire à une résistance qui n'existerait pas, ce qui est exactement le défaut que ce projet passe son temps à corriger. |
| **SHA-256** | Aucun défaut de sécurité. BLAKE3 est simplement plus rapide et sa crate est plus légère, à garantie égale pour cet usage. Le choix serait défendable dans l'autre sens. |
| **Signature asymétrique par entrée** | La clé privée devrait vivre quelque part sur la machine, donc être lisible par un attaquant SYSTEM : même limite, pour un coût bien supérieur. |
| **Expédition immédiate vers une ancre externe** | C'est la vraie réponse à A2, et elle est déjà prévue — SEC-04/05/06, Phase 3. Elle ne remplace pas le chaînage local, elle le complète. |

## Conséquences

### Ce que ça nous donne

La chaîne détecte désormais, avec une certitude cryptographique :

- la **corruption accidentelle** d'un enregistrement, disque ou logiciel ;
- la **troncature** du journal, y compris la suppression de sa première entrée,
  puisque `verify_chain` refuse la séquence vide et exige l'ancrage ;
- la **réécriture par un attaquant non privilégié**, qui ne peut pas recalculer
  les empreintes suivantes sans droit d'écriture sur tout le journal ;
- le **déplacement d'une frontière entre deux champs**, puisque chaque champ
  reste préfixé de sa longueur.

### Ce que ça nous coûte

Une dépendance de plus, sur un crate qu'il faut suivre. Et un changement
d'empreinte : les journaux écrits avec FNV-1a ne se vérifient plus. En Phase 0
aucun journal n'est encore persisté, donc le coût est nul aujourd'hui — mais il
ne le sera plus jamais après la 0.5, ce qui est une raison de faire ce
remplacement maintenant plutôt que plus tard.

### Ce que ça ne garantit pas — à lire avant de citer cette ADR

**Un attaquant qui obtient SYSTEM refabrique la totalité de la chaîne.** Il
réécrit les entrées qu'il veut, recalcule chaque empreinte depuis l'ancrage
public, et `verify_chain` répond « intacte ». Aucune trace, aucun signal.

Cette limite n'est pas un détail d'implémentation : c'est le cas d'usage
principal d'un journal d'audit. Elle doit figurer dans la documentation
utilisateur, au même titre que les autres angles morts du §5 du modèle de
menace. Un utilisateur qui croit son journal infalsifiable prendrait des
décisions sur une base fausse, et le brief l'interdit explicitement.

### Ce que ça ferme

Rien. Le passage à une empreinte clée reste ouvert, et l'ADR de Phase 2 pourra
reprendre exactement la même structure de matériau — étiquette de domaine,
champs préfixés de leur longueur — en changeant seulement la fonction. C'est
d'ailleurs une raison de garder l'étiquette de version `ks-journal-v1` dans le
matériau haché : elle permettra de distinguer les deux régimes sans ambiguïté.
