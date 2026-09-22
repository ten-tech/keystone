# Journal des versions

## v0.1.0 : « Observer et décrire », le 2026-09-23

Première version publiée. Keystone sait **lire** un poste de travail Windows 11,
**dire** ce qu'on veut qu'il soit, et **mesurer** l'écart entre les deux. Il ne
sait rien changer, et cette version le dit en toutes lettres plutôt que de le
laisser découvrir.

Elle clôt les phases 0 et 1 de la feuille de route, qui en compte six. Les quatre
suivantes, de la convergence à l'essaimage, ne sont pas commencées.

### Ce que le produit fait

**Il relève 143 items** sur six domaines, sans élévation de privilège :
configuration, inventaire, sécurité, espace, mises à jour, virtualisation. Chaque
item porte sa finalité et son risque, faute de quoi il ne serait pas affichable
(principe P6).

**Il adopte l'état lu comme référence.** `ks import` écrit `workstation.yaml`
depuis ce que la machine porte, sans qu'aucun formulaire soit à remplir. Le
fichier est versionnable en git, et le schéma JSON qui le décrit est **généré
depuis les types du produit**, donc incapable de diverger de ce qu'il relit.

**Il mesure l'écart.** `ks diff` publie quatre verdicts, ligne à ligne : conforme,
écart, incomparable, non contraint. Un item dont la lecture a échoué n'est jamais
compté parmi les conformes : ce qu'on n'a pas su lire ne se range pas dans une
catégorie rassurante.

**Il tolère un écart, avec sa raison et son échéance.** `ks accept` exige les
deux, et le typage les rend obligatoires. Un écart toléré **reste publié en
écart**, annoté de son échéance : tolérer n'est pas masquer, et à la date inscrite
l'écart redevient actif sans qu'aucune commande soit lancée. Les tolérances qui ne
couvrent plus rien se dénoncent elles-mêmes.

**Il consigne les décisions** dans un journal chaîné par empreintes BLAKE3, où
chaque champ est préfixé de sa longueur. La portée exacte de ce que le chaînage
détecte est affichée par la commande elle-même, y compris ce qu'il ne détecte pas.

**Il se regarde de deux façons** : une interface de bureau autonome, qui n'est pas
un navigateur et ne contacte aucun serveur, et un rapport HTML d'un seul fichier,
sans police distante ni script externe.

### Ce que le produit ne fait pas, et ne prétend pas faire

**Il n'écrit rien sur l'état de la machine** : ni registre, ni service, ni
politique, ni fichier d'un autre programme. Il écrit ses propres données et le
fichier d'état désiré, quand on le lui demande, jamais par omission.

**Il ne cherche aucun logiciel malveillant.** L'antivirus et l'analyse de malware
figurent nommément parmi ce que ce projet refuse. Il observe en revanche si les
défenses ont été éteintes, et quand l'outil de suppression livré par Windows
Update est passé pour la dernière fois.

**Il ne calcule aucun score de posture composite.** Un indicateur qu'on ne sait
pas déplier en ses composantes exactes n'est pas affichable.

**Il ne converge pas.** Les commandes qui écriront un jour sont déclarées et
refusent de s'exécuter, avec un code de sortie distinct du succès comme de
l'erreur.

### Les limites mesurées, pas supposées

Dix lectures restent hors de portée, chacune pour une raison relevée sur la
machine plutôt que devinée : le TPM et BitLocker par volume exigent une élévation
que la ligne de commande n'a jamais ; l'usure des disques passe par un appel
système qui demanderait du code non sûr ; la cohérence de l'horloge suppose une
référence externe, donc du réseau.

Le poste de référence tourne sous une édition **Famille** de Windows 11, où
Hyper-V n'existe pas. Un des quatre mécanismes d'instantané annoncés y est donc
indisponible, et la protection système y est désactivée. Le produit le relève et
l'affiche au lieu de promettre un filet qu'il n'a pas.

**Le critère de sortie de la phase 1 n'est pas atteint** : il demande sept jours
de dérive suivie sans faux positif inexpliqué, ce qui suppose un sondage régulier
que seul l'utilisateur peut planifier. Keystone ne créera jamais cette tâche
lui-même, une tâche planifiée étant une écriture système.

### Sous le capot

Vingt-quatre décisions d'architecture écrites au moment où elles étaient prises,
jamais en archéologie. Deux d'entre elles ont été amendées ou remplacées par une
suivante ; aucune n'a été effacée.

288 tests, dont la majorité sont des **barrières de conception** plutôt que des
tests de comportement : elles cassent la compilation quand quelqu'un enfreint un
principe. Chacune a été éprouvée par falsification, c'est-à-dire en injectant
réellement le défaut qu'elle prétend interdire et en vérifiant qu'elle tombe.

La vérification continue refuse un lien mort entre documents, un décompte de tests
qui ment, une couleur qui dérive de sa définition, une formulation proscrite par
le glossaire, et une documentation qui ne se construit pas sans avertissement.
