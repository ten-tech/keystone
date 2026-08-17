# ADR-0021 — Ce qu'un instantané sait défaire, et ce qu'il ne rattrape pas

- **Statut** : Proposé le 2026-08-17
- **Date** : 2026-08-17
- **Exigences concernées** : D3-07, D3-10, D3-15, D6-05, D9-02, D4-04, NF-07, P2, P3, P6, P7, SEC-08
- **Précise** : [ADR-0006](0006-fermer-les-verbes-a-parametres-libres.md) (les verbes
  n'acceptent pas de chemin libre), [ADR-0020](0020-ou-vit-une-derive-acceptee.md)
  (dont l'insertion chirurgicale attendait « un retour arrière que Keystone sait
  fabriquer lui-même »), [ADR-0004](0004-chainage-du-journal.md) (ce que l'empreinte
  couvre)
- **Ce lot n'ajoute aucun verbe à `ks-broker`.** Il décrit ce qu'un verbe devra
  pouvoir exiger avant d'exister. Les quatre questions du modèle de menace ne
  s'appliquent donc pas à ce document, et l'écrire ici évite qu'un relecteur les
  cherche.

## Contexte

### Pourquoi cette décision précède le broker et le premier verbe

P3 ne dit pas que la réversibilité est souhaitable : il en fait un **critère
d'admission**. Écrire un verbe de convergence avant de savoir l'annuler, ce
serait livrer un verbe inadmissible à l'instant de sa naissance, puis découvrir
la contrainte trop tard, quand il faudra choisir entre retirer la fonctionnalité
et affaiblir le principe. C'est pour cette raison que le moteur d'instantanés est
le premier livrable de la Phase 2, avant le service privilégié et avant la
moindre écriture.

Il y a une seconde raison, moins noble et tout aussi décisive : un filet dont on
surestime la portée est pire qu'une absence de filet. L'utilisateur applique en
confiance ce qu'il aurait refusé, et découvre l'étendue réelle de la couverture
au moment exact où il a besoin du retour arrière.

### Ce que le dépôt promet aujourd'hui

Trois endroits décrivent le filet, et les trois disent la même chose :

| Où | Ce qui y est écrit |
|---|---|
| [Feuille de route](../07-FEUILLE-DE-ROUTE.md), Phase 2 | « Moteur d'instantanés : point de restauration, checkpoint Hyper-V, `wsl --export`, export de registre » |
| [Cahier des charges](../01-CAHIER-DES-CHARGES.md), D3-07 | « point de contrôle Hyper-V, `wsl --export`, ou point de restauration selon la cible » |
| [Glossaire](../09-GLOSSAIRE.md), « Instantané ≠ Sauvegarde » | exemples : « point de contrôle Hyper-V, `wsl --export` » |

Le code en porte déjà la forme. `ks_core::SnapshotKind` énumère six natures,
`typical_rollback_seconds` annonce un coût de retour arrière (20, 200, 5 ou 600
secondes selon la nature), `Snapshot` porte une taille et une empreinte en champs
obligatoires, et `Plan::waves_without_safety_net` signale toute vague qui écrit
sans instantané. Le modèle de données du cahier (§9.2) résume : « `Snapshot` :
point de retour ; type, cible, empreinte, taille, expiration ».

Rien de tout cela n'a jamais été confronté à une machine.

### Les mesures du 2026-08-17, sur le poste de référence

Toutes les commandes ont été passées **sans élévation**, ce qui est l'état normal
de la CLI (SEC-01), et **sans rien écrire sur le système**.

| Question | Commande | Résultat |
|---|---|---|
| Édition du système | `(Get-CimInstance Win32_OperatingSystem).Caption` | `Microsoft Windows 11 Famille` |
| Édition, seconde lecture | `Get-ItemProperty 'HKLM:\...\CurrentVersion'` | `EditionID = Core`, `DisplayVersion = 25H2`, `CurrentBuild = 26200`, `UBR = 9168` |
| Élévation du processus | `IsInRole(Administrator)` | `False` |
| Hyper-V, module | `Get-Module -ListAvailable Hyper-V` | absent |
| Hyper-V, applets | `Get-Command Checkpoint-VM`, `Get-VM` | absents |
| Hyper-V, fonctionnalité | `Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V-All` | « L'opération demandée nécessite une élévation » |
| Points de restauration | `Get-ComputerRestorePoint` | « Accès refusé » |
| Points de restauration, par WMI | `Get-CimInstance -Namespace root/default SystemRestore` | « Accès refusé » |
| Espace des clichés | `vssadmin list shadowstorage` | refusé, code de sortie 2 |
| Protection système | `HKLM:\...\SystemRestore` | `RPSessionInterval = 0`, `SystemRestorePointCreationFrequency = 0`, `SRInitDone = 1` |
| Tâche planifiée `SR` | `Get-ScheduledTask -TaskPath '\Microsoft\Windows\SystemRestore\'` | `Ready` |
| WSL | `wsl --version`, `wsl --list --verbose` | 2.5.7.0 ; `Debian` (en marche), `docker-desktop` (en marche), `ks-lab` (arrêtée) |
| Taille des disques WSL | `Get-ChildItem *.vhdx` | `Debian` 54,19 Go · `ks-lab` 40,16 Go · `docker-desktop` 0,09 Go |
| Espace du volume système | `Win32_LogicalDisk` | 952,8 Go, dont 327,3 Go libres |
| Export de registre, petit | `reg export HKCU\...\Run` | code 0, 2 590 octets, 66 ms |
| Export de registre, ruche | `reg export HKLM\SYSTEM\CurrentControlSet\Services` | code 0, 15,57 Mo, 8 131 clés, 1 038 ms |
| Export de registre, large | `reg export HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion` | code 0, 122,49 Mo, 13 583 ms |
| Export d'une branche refusée | `reg export HKLM\SECURITY` | code 1, « Accès refusé » |
| Export d'une branche partiellement refusée | `reg export HKLM\SAM` | **code 0**, « L'opération a réussi », **138 octets** |
| Pourquoi cet export réussit | `(Get-Acl 'HKLM:\SAM').Access` | `BUILTIN\Utilisateurs : ReadKey` sur la racine ; `SYSTEM`, `BUILTIN\Administrateurs` et `CREATEUR PROPRIETAIRE` en `FullControl` |
| Pourquoi il ne contient rien | `Get-Item 'HKLM:\SAM'` puis `Get-Item 'HKLM:\SAM\SAM'` | la racine annonce 1 sous-clé ; la descente vers elle rend « Accès au registre demandé non autorisé » |

Cette ligne d'ACL n'est pas un détail d'érudition. Elle est la raison pour
laquelle le relevé ci-dessus dépend du jeton qui l'obtient : la même commande,
passée depuis une session plus restreinte, rend un refus franc et aucun fichier.
Sans elle, un lecteur qui rejouerait la commande dans six mois et obtiendrait
« Accès refusé » conclurait que ce document se trompe, alors que c'est son
contexte d'exécution qui diffère.

Deux mesures n'ont **pas** été faites, et rien ne les remplace : la durée et le
volume réels d'un `wsl --export` sur une distribution de 40 Go, et le coût en
temps et en espace d'un point de restauration. La première suppose une heure de
disque, la seconde suppose d'activer la protection système, c'est-à-dire d'écrire
sur l'hôte. Les deux sont donc **non mesurées**, et ce document ne les estime pas.

### Le piège de mesure, qui vaut d'être consigné

Trois lectures de la même chose donnent trois chaînes différentes. `Caption` par
WMI donne « Microsoft Windows 11 Famille ». `ProductName` en registre donne
« Windows 10 Home », valeur figée par l'éditeur et fausse sur les deux termes.
`sysinfo::System::long_os_version()`, que le collecteur d'inventaire emploie déjà
pour `inventory.os.name`, donne « Windows 11 Home ». Un moteur qui déciderait de
ses mécanismes disponibles en comparant des chaînes de ce genre se tromperait le
jour où l'une d'elles change. La disponibilité se mesure en interrogeant la
capacité, jamais en lisant un nom.

### Ce que ces mesures interdisent d'écrire

**Hyper-V est hors d'atteinte sur ce poste, et la documentation de l'éditeur le
confirme.** « The Hyper-V role **can't** be installed on Windows 10 Home or
Windows 11 Home », et les prérequis listent « Windows 10 (Pro or Enterprise), or
Windows 11 (Pro or Enterprise) » (Microsoft Learn, « Install Hyper-V in Windows
and Windows Server », mis à jour le 2026-02-16, consulté le 2026-08-17).

Le périmètre du produit (§4.1) vise Windows 11 Pro et Enterprise, si bien qu'un
point de contrôle Hyper-V n'est pas *impossible chez l'utilisateur cible*. Il est
impossible **ici**, sur la seule machine qui écrit le code, et il n'est pas
davantage éprouvé dans le labo : la [note sur la VM de labo](../06-VM-DE-LABO.md)
écrit noir sur blanc qu'Hyper-V dans l'invité reste « le point qui décide de la
suite », non mesuré à ce jour. Or NF-07 tranche cette situation sans ambiguïté :
« un chemin de retour arrière non testé est réputé inexistant ». Un mécanisme que
ni le poste de développement ni le labo ne savent produire ne peut donc pas
figurer parmi les filets sur lesquels une fonctionnalité s'admet.

**Le point de restauration système n'est pas davantage un acquis.** Six faits, et
aucun n'est confortable :

1. Sur ce poste, la protection système est **désactivée** (`RPSessionInterval` à
   0). Aucun point ne peut donc être pris tant qu'elle n'est pas activée, et
   l'activer est une écriture système qui réserve de l'espace disque.
2. La création exige une élévation : les deux lectures que la CLI a tentées sont
   refusées, donc la création l'est a fortiori.
3. Par défaut, la plateforme **saute** la création si un point a été créé dans
   les 24 heures : « If the key does not exist (default) and any restore points
   have been created in the last 24 hours, Windows skips creating this new
   restore point », et la fonction renvoie pourtant `TRUE` avec le numéro de
   séquence du point précédent (Microsoft Learn, « Calling SRSetRestorePoint »).
   L'applet `Checkpoint-Computer` porte la même limite : « Beginning in Windows 8,
   `Checkpoint-Computer` cannot create more than one system restore point each
   day ». **Un appel qui réussit ne prouve donc pas qu'un point a été pris.**
4. La rétention n'appartient pas à Keystone. « Windows 11, version 24H2 will
   retain system restore points for up to 60 days » (Microsoft, KB5060842, mise à
   jour du 2025-06-10), et la purge par pression d'espace existe de longue date.
   Le filet peut disparaître sans que personne l'ait demandé.
5. La couverture est **par extension de nom de fichier**, et la liste officielle
   des extensions surveillées contient `LOG`, `CONFIG`, `INI`, `JS`, `REG`, `DLL`,
   `EXE` (Microsoft Learn, « Monitored File Name Extensions », mis à jour le
   2026-01-28). Ce qui n'y figure pas n'est pas restauré, et ce qui y figure l'est
   **où qu'il vive** : revenir en arrière peut donc ramener un `.config` ou un
   `.js` d'un projet de travail à son état d'avant l'opération. La documentation
   ne dit nulle part que l'emplacement protège. Ce comportement n'a pas été
   vérifié par une restauration réelle, et il est écrit ici comme une question
   ouverte, pas comme un fait établi.
6. Ni la taille ni l'empreinte d'un point de restauration ne sont calculables.
   L'espace des clichés est global, partagé entre tous les points, et son relevé
   exige une élévation. Le modèle de données du cahier (§9.2) exige les deux.

### La mesure qui commande tout le reste

`reg export HKLM\SAM` renvoie **code 0**, affiche « L'opération a réussi », et
produit un fichier de **138 octets** contenant l'en-tête et une seule ligne,
`[HKEY_LOCAL_MACHINE\SAM]`. Aucune sous-clé, aucune valeur : les ACL les
refusaient, et l'outil n'en dit rien. La branche voisine, `HKLM\SECURITY`, dont
la racine elle-même est illisible, échoue proprement avec le code 1.

Autrement dit, la frontière entre « capturé » et « vide » n'est pas signalée par
le code de retour. Un moteur d'instantanés qui ferait confiance à ce code
inscrirait au journal un filet pris, l'afficherait dans le bandeau du plan, et
l'utilisateur appliquerait. Le fichier serait là, daté, de taille non nulle, et
ne contiendrait rien de ce qu'on croyait sauver.

Un contrôle croisé a d'ailleurs produit, sur une session plus restreinte de la
même machine, un refus franc là où la session utilisateur normale obtient une
réussite tronquée. Les deux relevés sont justes, et c'est précisément le point :
**ce qu'un export capture dépend du jeton du processus qui l'a produit**, donc du
contexte d'exécution, et non de la seule commande.

**Le broker verra plus que la CLI.** Il s'exécute en SYSTEM, là où `ks` s'exécute
sans élévation, si bien qu'un même export lancé par l'un et par l'autre ne
couvrira pas les mêmes clés. La conséquence est directe pour la Phase 2 : la
couverture d'un instantané **ne peut pas être déduite de ce que la CLI a pu
lire**, ni de ce qu'un scan non élevé a inventorié. Elle se relève sur
l'artefact, après coup, dans le contexte qui l'a produit. Un bac à sable, à
l'inverse, verra moins que la CLI, et aucun raisonnement a priori ne dit ce que
l'artefact contiendra.

La relecture ciblée n'est donc pas une précaution contre un cas particulier :
c'est la seule chose qui reste vraie quel que soit le contexte.

### Ce que le fichier `.reg` n'est pas

Un export de registre n'est pas une annulation. La documentation de `reg import`
dit « Copies the contents of a file that contains exported registry subkeys,
entries, and values into the registry », et ne mentionne aucune suppression : ce
que l'export ne contient pas subsiste. Réimporter après avoir **créé** une valeur
ne la retire donc pas, et le retour arrière est faux là où il paraît complet.

La remise en état par remplacement existe, mais elle a un tout autre visage :
`RegRestoreKey` « replaces the keys and values below the specified key », exige
les privilèges `SE_RESTORE_NAME` **et** `SE_BACKUP_NAME`, échoue si une sous-clé
est ouverte, et l'éditeur recommande explicitement de lui préférer le service de
cliché instantané pour l'état système (Microsoft Learn, `RegRestoreKeyW`, consulté
le 2026-08-17). C'est une opération privilégiée, brutale, et dont la portée est
exactement celle de l'arbre restauré, ni plus ni moins.

### Ce que `wsl --export` sait faire, mesuré et documenté

La documentation dit : « Exports a snapshot of the specified distribution as a
new distribution file. Defaults to tar format ». Sur ce poste, l'aide du binaire
2.5.7.0 expose l'option sous la forme `--format <tar|tar.gz|tar.xz|vhd>`, là où
la page en ligne documente `--vhd` : la forme exacte de l'option se relève sur la
machine, elle ne se recopie pas d'une page.

Le retour arrière, lui, n'est pas symétrique. `wsl --import` crée une **nouvelle**
distribution ; revenir à l'état d'avant suppose donc de désinscrire l'existante,
et « Once unregistered, all data, settings, and software associated with that
distribution will be permanently lost ». Entre les deux gestes, il n'existe rien.
Sur `Debian`, qui pèse 54,19 Go, cela signifie qu'un retour arrière détruit
d'abord ce qu'il prétend restaurer, et que tout ce qui a été écrit dans la
distribution depuis l'export est perdu sans avertissement.

## Décision

### 1. Un instantané n'est réputé pris que lorsque Keystone a relu son contenu et y a retrouvé les items du plan

Le code de retour de l'outil qui produit l'artefact ne vaut rien : la mesure sur
`HKLM\SAM` le démontre pour le registre, et la documentation de
`SRSetRestorePoint` le démontre pour le point de restauration, qui renvoie
`TRUE` en ayant sauté la création. La prise se conclut donc par une **relecture
ciblée** : l'artefact est ouvert, et l'on vérifie qu'il porte la valeur courante
de **chacun** des items que la vague s'apprête à changer.

Cette formulation est plus exigeante qu'elle en a l'air, et c'est voulu. Elle ne
demande pas un fichier bien formé, ni une taille non nulle, ni une empreinte : un
fichier de 138 octets est bien formé. Elle demande la présence des chemins
exacts que le plan cite. C'est le même geste que celui de l'ADR-0020, dont
l'insertion chirurgicale relit le document produit avec le lecteur du produit
avant de le poser sur le disque.

Un instantané dont la relecture échoue n'est pas un instantané dégradé : il
n'existe pas, la vague est sans filet, et le plan s'arrête là.

### 2. Trois degrés de preuve, et un seul autorise l'automatisation

| Degré | Ce qui est prouvé | Ce qu'il autorise |
|---|---|---|
| **Existence** | l'artefact est là, daté, attribué à un plan | rien |
| **Relecture** | l'artefact contient la valeur courante de chaque item visé (décision n° 1) | l'application, avec présence humaine (SEC-08) |
| **Restauration éprouvée** | le retour a été réellement joué sur une cible équivalente, et l'état d'après a été comparé à l'état d'avant par un scan | l'application automatique, donc l'ordonnanceur et D3-10 |

C'est la traduction littérale de NF-07 : « un chemin de retour arrière non testé
est réputé inexistant ». Le degré se porte sur la **classe de mécanisme**, pas
sur l'instantané individuel : on n'éprouve pas chaque export de registre en le
restaurant, on éprouve une fois que l'export de registre sait restaurer, dans le
labo, et l'on rejoue cette épreuve à chaque changement de version du produit ou
du système, comme on rejoue une éval.

Conséquence directe et assumée : **au premier jour de la Phase 2, aucun mécanisme
n'est au degré trois.** Donc rien ne s'automatise, donc la convergence de la
Phase 2 s'applique avec présence humaine, et le rollback automatique promis par
D3-10 attend son épreuve. C'est un ralentissement réel du calendrier, et c'est
exactement ce que P3 coûte quand on le prend au sérieux.

### 3. Deux rangs de filet, et le filet de plateforme n'est jamais le chemin nominal

**Rang 1, l'annulation que Keystone fabrique lui-même.** Avant de changer un
item, le moteur écrit ce qu'il faut pour le remettre dans son état antérieur : la
valeur courante relevée, la branche de registre exacte que l'item occupe, la
copie du fichier exact que l'action modifie. La portée est celle de l'item, la
prise est de l'ordre de la milliseconde, et la mesure le confirme : 2 590 octets
et 66 ms pour une branche réelle.

**Rang 2, le filet de plateforme.** Le point de restauration système, quand il
est disponible et éprouvé, et l'export d'une distribution WSL. Sa portée dépasse
de très loin ce que le plan touche.

Le rang 2 est un **dernier recours**, jamais le retour arrière nominal. La raison
n'est pas le coût, c'est la nature : revenir par un filet large ramène aussi ce
qui n'appartenait pas à Keystone. Pendant l'opération, le système d'exploitation,
les navigateurs, les services et l'utilisateur ont continué d'écrire. Un retour
de rang 2 **détruit** ces écritures légitimes. Un filet qui, pour réparer un
item, en efface cent autres n'est pas un filet : c'est une seconde avarie, et
elle survient au pire moment, celui où l'on répare la première.

Un plan qui ne dispose que du rang 2 pour une action donnée doit le dire dans le
bandeau, en nommant ce que le retour emporterait.

### 4. L'unité d'instantané est l'action pour le rang 1, le plan pour le rang 2, et il n'y a rien entre les deux

Le modèle de données (§9.2) décrit un instantané par « type, cible, empreinte,
taille, expiration », ce qui suppose implicitement une granularité libre. Les
mécanismes ne l'offrent pas.

Le rang 1 se prend **par action**, parce qu'il est ciblé par construction et que
son coût est négligeable. Le rang 2 se prend **par plan**, parce qu'aucun de ses
mécanismes ne sait faire moins : le point de restauration porte sur le volume, sa
fréquence est bornée à un par 24 heures par défaut, et son espace est partagé ;
l'export WSL porte sur la distribution entière.

La **vague** reste l'unité de jugement, celle après laquelle les tests de fumée
s'exécutent (D3-09), mais elle n'est pas une unité d'instantané. `Wave.snapshots`
devient donc la liste des filets **exigés** par la vague, et non des filets pris.

Un invariant en découle, et il est vérifiable : chaque action d'une vague qui
écrit doit être couverte par au moins un instantané dont la couverture contient
le chemin de l'item visé. Une vague dont une seule action est découverte est une
vague sans filet.

### 5. La disponibilité du filet est un item observable, donc un écart, jamais une exception d'exécution

Sur ce poste, la protection système est désactivée. Découvrir cela au moment
d'appliquer serait doublement fautif : trop tard, et au mauvais endroit. Le
moteur relève donc la disponibilité de chaque mécanisme **pendant le scan**,
comme n'importe quel autre fait de la machine, et l'absence de filet devient un
écart affiché par `ks diff`, avec sa finalité et son risque (P6).

**Keystone n'active jamais la protection système de lui-même**, ni au premier
lancement, ni en préalable d'un plan. Réserver de l'espace disque sur le volume
système est une décision de l'utilisateur, dont le coût est réel et durable.
Keystone la propose, l'explique, et attend. La règle vaut identiquement pour toute
fonctionnalité facultative du système dont un filet dépendrait.

### 6. Le modèle cesse d'affirmer ce qu'il ne sait pas produire

Trois corrections, toutes dans `crates/ks-core/src/snapshot.rs`, et toutes issues
des mesures ci-dessus.

**`SnapshotKind::HyperVCheckpoint` est retiré.** Ni le poste de référence ni le
labo ne savent le produire, et NF-07 réputé inexistant un chemin non testé. Une
variante qu'aucune machine du projet ne peut construire est une promesse
d'interface : elle apparaît dans le JSON, dans les libellés, dans la
documentation, et elle laisse croire à une couverture. Elle se réécrira le jour
où un poste Pro éprouvera le mécanisme, et ce jour-là elle vaudra quelque chose.

**`typical_rollback_seconds` disparaît sous sa forme actuelle.** Ses quatre
valeurs sont inventées ; aucune ne provient d'une mesure. Les afficher à
l'utilisateur comme un coût contredit P6, pour lequel « un chiffre inexplicable
est une décoration ». Le coût annoncé provient désormais de la mesure locale la
plus récente pour ce mécanisme sur cette machine, et **l'absence de mesure
s'affiche comme telle**, sans nombre. Le type doit rendre inconstructible
l'affichage d'une durée qu'on n'a pas mesurée.

**La taille et l'empreinte cessent d'être obligatoires pour tout mécanisme.**
Elles sont mesurables pour ce que Keystone détient, et hors d'atteinte pour un
point de restauration, dont la taille n'est pas attribuable et l'empreinte pas
calculable. Elles deviennent des propriétés portées **par le mécanisme**, et non
par la structure commune. Le cahier §9.2 est corrigé dans le même sens.

**Et la même correction s'applique côté broker, sur un paramètre existant.**
`SnapshotSubject::LabVirtualMachine`, documenté comme « point de contrôle de la
machine virtuelle de laboratoire », est retiré. Il est faux deux fois : le
mécanisme suppose Hyper-V, absent de ce poste, et le labo réel n'est pas une
machine Hyper-V mais un invité QEMU dans une distribution WSL
([note de labo](../06-VM-DE-LABO.md)). Au demeurant, la remise à zéro du labo se
fait depuis l'hôte, par les scripts du dépôt, et n'a aucune raison de traverser
le composant privilégié. Ce retrait touche la surface d'API du broker, ce qui
exige une décision documentée : c'est celle-ci. Il fera casser la compilation aux
endroits où le `match` exhaustif de la barrière SEC-02 vit, ce qui est le
comportement recherché.

### 7. Keystone ne purge que ce qu'il détient, jamais avant la fenêtre de vérification, jamais un filet attaché à un plan en échec

L'expiration se traite en trois règles, dont la troisième est celle qui compte.

1. **Ce que Keystone ne détient pas, il ne le purge pas, et il ne promet rien de
   sa durée de vie.** Un point de restauration vit selon les règles de la
   plateforme, dont la limite de 60 jours annoncée par l'éditeur et la purge par
   pression d'espace. L'interface affiche cette dépendance plutôt que de la
   masquer derrière une échéance que Keystone n'honorerait pas.
2. **Ce que Keystone détient vit tant que le plan n'est pas clos**, puis sept
   jours après la dernière vérification réussie. Sept jours est la fenêtre déjà
   retenue par le projet pour observer une dérive (critère de sortie de la
   Phase 1) : c'est un écho, pas un nombre neuf. Valeur en dur et commentée, sans
   réglage : un second cas d'usage la rendra configurable, pas avant.
3. **Un instantané attaché à un plan dont un test de fumée a échoué n'est jamais
   purgé automatiquement.** C'est le cas précis où la purge détruirait le filet
   juste avant qu'on en ait besoin, et la seule prévention fiable est de ne pas
   compter sur la vigilance. Sa suppression exige un geste humain explicite.

Et rien n'est effacé : ce qui doit disparaître part en quarantaine avec sa durée
de vie, selon D4-04, dont la règle « quarantaine, jamais suppression » s'applique
au filet comme au reste. La purge définitive exige Windows Hello.

*Note de correction : c'est **D4**, « Espace et propreté », qui porte l'espace
disque et la quarantaine. D7 est le domaine « Secrets et identités ».*

### 8. Ce qu'aucun mécanisme ne couvre, et qui s'affiche avant l'application

C'est la section que l'on oublie, et c'est celle qui tient P3 honnête. Aucune des
lignes suivantes n'est rattrapée par un instantané, quel qu'en soit le rang.

| Angle mort | Pourquoi aucun instantané ne le couvre |
|---|---|
| **Ce qui a quitté la machine** | une publication, une synchronisation, un `git push`, un envoi vers un service tiers. La copie distante ne revient pas |
| **Ce qui a été consommé** | une mise à jour de firmware appliquée, une licence activée, un secret pivoté, une migration de format de base de données |
| **Le firmware et les réglages d'amorçage** | UEFI, démarrage sécurisé, état du TPM. Effacer un TPM détruit les protecteurs BitLocker qui en dépendent, et rien de local ne les reconstruit |
| **Le chiffrement de volume** | suspendre, reprendre, changer de protecteur ou rechiffrer ne s'annule pas en revenant à un état antérieur du volume |
| **Les fichiers de travail** | le point de restauration sélectionne par extension. Ce qui n'est pas surveillé n'est pas restauré, et ce qui l'est peut être ramené en arrière alors qu'on ne le voulait pas |
| **Les écritures concurrentes** | tout ce que le système et l'utilisateur ont écrit pendant l'opération. Un retour de rang 2 les emporte |
| **Le temps d'exposition** | une protection désactivée pendant la fenêtre d'application l'a été réellement. Revenir en arrière ne rétablit pas le passé |
| **Ce qu'une politique gérée réappliquera** | la MDM est souveraine (P10). Un retour arrière sur un item géré sera écrasé au cycle suivant, et l'oscillation coûte plus que l'écart |
| **Un état intermédiaire avant redémarrage** | certaines convergences ne prennent effet qu'au redémarrage. Entre l'écriture et lui, l'état n'est ni l'ancien ni le nouveau, et il n'est pas restaurable atomiquement |
| **Les secrets exposés** | une valeur lue par un tiers reste lue. Le retour arrière ne la reprend pas |

Cette liste vit dans le produit, pas seulement dans ce document : elle s'affiche
au moment du plan, restreinte aux lignes qui concernent les actions de ce plan.
Une action dont le seul angle mort applicable est décisif ne s'automatise pas.

### 9. Ce que le moteur exige du broker, en capacités, et la contrainte qui les borne

Le moteur n'invente aucun verbe et n'en propose aucun. Il énonce ce dont il aura
besoin, à charge pour l'ADR du broker de décider si, et comment, cela s'exprime.

Deux verbes figurent d'ailleurs **déjà** dans l'énumération, sans mise en œuvre,
et l'ADR-0006 y a posé l'essentiel des bornes ci-dessous : le sujet d'une prise
est une énumération sans champ, et l'identifiant d'un retour est un type validé à
la construction, à jeu de caractères clos, « jamais concaténé à un chemin ». Ce
document ne les renomme pas, n'en ajoute aucun, et confirme ce raisonnement. Il
ne touche qu'un point, le sujet de laboratoire, pour la raison écrite en
décision n° 6.

Quatre capacités, dont la dernière est d'une autre nature que les trois premières :

1. **Demander à la plateforme la création d'un point de retour de volume**, et
   recevoir l'identifiant que **la plateforme** attribue, jamais un identifiant
   choisi par l'appelant. Cette capacité doit distinguer la création effective
   d'une création sautée, ce que l'interface système ne fait pas d'elle-même
   (décision n° 1).
2. **Lire l'inventaire des points de retour existants et leur état**, refusé sans
   élévation, donc inaccessible à la CLI seule.
3. **Copier vers un artefact détenu par Keystone** une branche de registre ou un
   fichier désignés par l'item concerné, et **remettre cette cible dans l'état que
   l'artefact décrit**.
4. **Restaurer**, qui n'est pas la symétrique anodine de la précédente.

La quatrième capacité doit répondre à la question éliminatoire du modèle de
menace avant toute autre, et la réponse n'est pas évidente : **remettre une
branche de registre arbitraire dans un état arbitraire équivaut à une écriture
arbitraire du registre**, donc à une exécution de code par détour, par exemple à
travers les options d'exécution de fichier image. C'est précisément ce que
l'interdiction de `SetRegistryValue sans ACL` vise.

Trois bornes, qui doivent tenir ensemble, faute de quoi la capacité ne passe pas :

- l'artefact restaurable est **produit et détenu par Keystone**, dans un
  emplacement dont le broker est propriétaire ;
- il se désigne par un **identifiant d'instantané**, jamais par un chemin de
  fichier fourni par l'appelant. C'est la règle que l'ADR-0006 a déjà posée pour
  les verbes de configuration : les paramètres sont des énumérations, pas des
  chemins ;
- la portée de la remise en état est **bornée par la couverture enregistrée de
  l'instantané**, elle-même bornée par ce que le plan a effectivement touché. Une
  restauration qui déborderait sa couverture est refusée.

Sans ces trois bornes, la capacité de retour arrière serait l'outil d'attaque le
plus efficace du poste, et elle porterait le nom rassurant de « filet de
sécurité ».

### 10. La décision vit dans `ks-core`, l'exécution vit derrière le broker

Ce qui relève du **jugement** est portable, testable sans privilège et sans
machine Windows : quel filet convient à quelle action, la couverture d'un
instantané, le degré de preuve atteint, la rétention, l'invariant de couverture
d'une vague. Cela vit dans `ks-core`, où les tests tournent déjà sur la CI Linux.

Ce qui relève de l'**effet** est spécifique, privilégié, et non testable ailleurs
que dans le labo : la création du point de retour, la lecture de l'inventaire,
l'écriture et la remise en état d'un artefact. Cela vit derrière le broker.

La frontière se vérifie : `ks-core` ne dépend d'aucune interface système, et la
décision de prendre un filet se calcule à partir de faits déjà collectés.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| **Garder `HyperVCheckpoint` en le documentant comme non disponible ici** | c'est l'état que le dépôt a déjà condamné pour la dérive acceptée : une déclaration qui rassure et n'agit pas, « le pire des trois états possibles » (ADR-0020). La variante apparaît dans le JSON, dans les libellés et dans la doc, et personne ne relit la note. Elle revient le jour où elle est éprouvée |
| **Développer le mécanisme Hyper-V à l'aveugle, en le testant plus tard sur une machine Pro** | on écrirait du code qu'aucune CI ne peut exécuter, contre une interface qu'on ne peut pas observer. Le projet a déjà payé quatre fois le prix des versions supposées plutôt que compilées (ADR-0016) |
| **Se reposer sur le seul point de restauration système, comme le font les installeurs** | il est désactivé sur ce poste, refusé sans élévation, borné à un par 24 heures par défaut, purgé par la plateforme au bout de 60 jours, sélectif par extension, et sa taille comme son empreinte sont hors d'atteinte. Il ne satisfait aucune ligne du modèle §9.2 |
| **Prendre le filet de plateforme comme retour arrière nominal, parce qu'il couvre plus** | il couvre trop. Revenir en arrière emporte les écritures concurrentes légitimes, donc répare un item en en cassant d'autres, sans que l'utilisateur l'ait demandé ni compris |
| **Un instantané par vague, aligné sur les tests de fumée** | séduisant par symétrie, faux en pratique : aucun mécanisme de rang 2 ne sait se prendre trois fois de suite dans la même heure, et le rang 1 n'a aucune raison d'attendre la vague, sa granularité naturelle étant l'item |
| **Un instantané global par exécution, pour simplifier** | rendrait impossible l'épinglage du composant fautif que D3-10 exige : revenir en arrière annulerait aussi les actions qui ont réussi, et le compte rendu ne pourrait plus nommer le responsable |
| **Faire confiance au code de retour de l'outil qui produit l'artefact** | mesuré faux : `reg export HKLM\SAM` renvoie 0 pour 138 octets vides, et `SRSetRestorePoint` renvoie `TRUE` en ayant sauté la création. C'est le mode de défaillance le plus dangereux, celui qui produit un filet qui se déclare pris |
| **Vérifier l'artefact par sa taille ou par une empreinte** | une empreinte prouve qu'un fichier n'a pas bougé, pas qu'il contient quelque chose d'utile. Les 138 octets ont une empreinte parfaitement valide. Seule la relecture des chemins visés discrimine |
| **Éprouver la restaurabilité en restaurant réellement, à chaque prise** | sur l'hôte, c'est un redémarrage et une remise en état par instantané à chaque convergence, donc l'inverse de P7. L'épreuve porte sur la classe de mécanisme, dans le labo, et se rejoue à chaque changement de version |
| **Se contenter de vérifier que l'artefact existe** | c'est le degré un, et NF-07 le réfute en une phrase. Un filet non éprouvé est réputé inexistant, ce qui n'interdit pas de le prendre, mais interdit de s'y fier pour automatiser |
| **Activer la protection système au premier lancement, pour garantir un filet** | une écriture système non demandée, qui réserve de l'espace disque durablement, sur un produit dont la promesse d'adoption est de ne rien écrire avant la Phase 2. Le filet manquant est un écart affiché, pas une initiative |
| **Purger les instantanés à date fixe, sans regarder l'issue du plan** | détruirait le filet précisément dans le cas où il sert, celui du plan dont un test de fumée a échoué et dont le retour arrière n'a pas encore eu lieu |
| **Ne jamais purger, et laisser l'utilisateur décider** | les exports de registre larges pèsent lourd : 122,49 Mo pour une seule branche mesurée. Un filet immortel finit par consommer l'espace que D4 s'emploie à rendre, et le domaine qui récupère l'espace ne peut pas être celui qui le gaspille |
| **Purger en supprimant** | contredit D4-04, « quarantaine, jamais suppression », qui vaut pour le filet comme pour le reste |
| **Confier l'artefact au moteur de sauvegarde D6** | confondrait les deux catégories que le glossaire sépare : l'instantané protège la machine, la sauvegarde protège le travail. Les types `Snapshot` et `BackupSet` sont sans conversion, et c'est l'un des invariants les plus utiles du modèle |
| **Faire porter au moteur la restauration d'un artefact désigné par son chemin** | équivaudrait à une écriture arbitraire du registre, donc à une exécution de code par détour. La désignation par identifiant d'instantané est ce qui sépare un filet d'une porte dérobée |
| **Laisser le moteur d'instantanés vivre entièrement dans le broker** | le jugement deviendrait intestable sur la CI, et le composant le plus privilégié du produit grossirait de toute la logique de rétention et de couverture. Le broker est petit exprès |

## Conséquences

### Ce que ça nous donne

P3 devient vérifiable au lieu d'être professé. Une action qui n'a pas de filet
couvrant son item ne se planifie pas, un filet non relu ne compte pas, un
mécanisme non éprouvé n'autorise pas l'automatisation, et chacune de ces trois
règles casse un test quand on l'enfreint.

Le mode de défaillance le plus dangereux disparaît. Un filet qui se déclare pris
sans avoir capturé quoi que ce soit est le seul incident qui transforme un outil
de prudence en outil de dégât ; la relecture ciblée le rend inatteignable par le
chemin nominal.

L'utilisateur reçoit, avant d'appliquer, la portée réelle du retour arrière et la
liste de ce qu'il ne rattrapera pas. C'est P6 appliqué au filet lui-même, et
c'est la différence entre un produit qui inspire confiance et un produit qui la
capte.

Le rang 1 est presque gratuit : 2 590 octets et 66 ms sur une branche réelle.
Le filet nominal ne coûte donc ni temps ni espace notables, ce qui retire le seul
argument qu'on aurait pu opposer à sa systématisation.

### Ce que ça nous coûte

**Le rollback automatique de D3-10 recule.** Aucun mécanisme n'atteint le degré
trois au premier jour, donc la Phase 2 s'applique avec présence humaine, et le
critère d'acceptation A3, qui exige une annulation automatique en moins de
quinze minutes sans intervention, ne peut pas être atteint tant que l'épreuve de
restauration n'a pas eu lieu dans le labo. C'est le coût direct de prendre P3 au
sérieux, et il vaut mieux le payer ici qu'au premier incident.

**Une épreuve de labo devient un prérequis de livraison**, à rejouer à chaque
changement de version du produit ou du système. C'est une charge récurrente, du
même ordre que les évals d'un prompt.

**Le modèle perd une variante et gagne de la nuance.** Retirer
`HyperVCheckpoint` casse la compilation de ses appelants, dont un test, ce qui
est le comportement voulu. Rendre la taille et l'empreinte propres au mécanisme
complique la structure commune, au bénéfice de l'honnêteté.

**Deux mesures manquent toujours** : la durée et le volume d'un `wsl --export`
réel, et le coût d'un point de restauration. Elles se prendront dans le labo, et
tant qu'elles manquent, l'interface affiche « non mesuré » plutôt qu'un nombre.

### Ce que ça corrige dans la documentation, et qui part au même commit

**La feuille de route, Phase 2, est fausse sur un point.** La ligne « Moteur
d'instantanés : point de restauration, checkpoint Hyper-V, `wsl --export`, export
de registre » doit devenir : « Moteur d'instantanés : annulation ciblée par item
(export de branche de registre, copie de fichier), et filet de plateforme en
dernier recours (point de restauration système, `wsl --export`). Le point de
contrôle Hyper-V est hors d'atteinte du poste de référence, en édition Famille, et
non éprouvé dans le labo : il revient quand une machine sait le produire. »

**Le cahier des charges, D3-07, est faux du même point.** « Instantané avant
application : point de contrôle Hyper-V, `wsl --export`, ou point de restauration
selon la cible » doit devenir « instantané avant application, ciblé sur ce que
l'action touche quand Keystone sait le fabriquer, et filet de plateforme sinon ».
La seconde phrase de l'exigence, « une cible sans instantané possible n'est pas
éligible à l'application automatique », est confirmée et renforcée : une cible
dont le filet n'est pas **éprouvé** ne l'est pas davantage.

**Le glossaire est faux sur son exemple.** La ligne « Exemple : point de contrôle
Hyper-V, `wsl --export` » devient « export de la branche de registre d'un item,
`wsl --export` ». La distinction entre instantané et sauvegarde, elle, est
intacte et confirmée.

**Le cahier des charges, §9.2, décrit une entité que les mécanismes ne peuvent
pas honorer.** « `Snapshot` : point de retour ; type, cible, empreinte, taille,
expiration » doit préciser que l'empreinte et la taille sont propres au mécanisme
et absentes pour un point de restauration, et que l'expiration n'engage Keystone
que pour ce qu'il détient.

**La documentation d'architecture cite `Snapshot` parmi le vocabulaire de
`ks-core`** et le décrit comme un type sans conversion vers `BackupSet` : cette
propriété est intacte, mais la description des champs suit la correction du §9.2.

**La note sur la VM de labo gagne une conséquence.** Son point ouvert, Hyper-V
dans l'invité, ne décide plus seulement du confort de développement : il décide
de la réintroduction d'un mécanisme d'instantané. À écrire là-bas, pour que la
mesure ne soit pas repoussée faute d'enjeu visible.

**D9-02 reste vraie mais devient conditionnelle.** « Instantané et retour arrière
par distribution, avec rétention configurable » suppose un export de 54,19 Go et
une désinscription destructive entre l'ancien et le nouveau. L'exigence est
maintenue, avec la mention de ce que le retour arrière détruit.

**D3-15 est affaiblie et doit le dire.** « Distros WSL et VM : automatisation plus
poussée assumée, l'instantané y étant instantané et le retour arrière de l'ordre
de 20 secondes » repose sur le point de contrôle Hyper-V. Pour WSL, ni l'instant
ni les vingt secondes ne sont mesurés, et l'ordre de grandeur des volumes rend
les deux improbables. L'exigence est conservée pour les VM, et suspendue pour
WSL jusqu'à mesure.

### Ce que ça ferme

Rien d'irréversible. Le mécanisme Hyper-V se réécrit en une variante et un
module le jour où une machine sait le produire et le labo l'éprouver, et ce
document dit exactement à quelle condition.

Ce qui se ferme, en revanche, et volontairement : l'idée qu'un instantané puisse
être pris sans être relu, qu'un filet non éprouvé autorise l'automatisation, et
qu'un artefact désigné par un chemin libre puisse être restauré par un composant
privilégié.

### Ce que ça ne garantit pas

**Le rang 1 ne couvre que ce que Keystone a su lire.** Un item dont la valeur
courante est illisible, refusée par une ACL ou absente, n'a pas d'annulation
ciblée : ADR-0008 a déjà tranché qu'un item illisible ne se compte pas parmi les
conformes, et il ne se compte pas davantage parmi les réversibles.

**Rien n'empêche un tiers d'écrire pendant l'opération.** Le retour arrière
ramène l'item dans l'état relevé avant l'action, pas dans l'état où il serait si
personne n'avait rien fait. Sur un poste d'ingénieur, l'écart entre les deux est
réel.

**L'adversaire A2 rend tout ceci sans objet.** Qui obtient SYSTEM détruit les
instantanés, réécrit le journal, et `verify_chain` répond « intacte » : la portée
exacte est écrite dans l'ADR-0004. Les instantanés figurent parmi les actifs à
protéger du modèle de menace, et rien de local ne les protège d'un adversaire
privilégié.

**La restauration éprouvée l'est sur une cible équivalente, jamais sur la
machine réelle de l'utilisateur.** Une équivalence de labo n'est pas une identité,
et un système d'exploitation diverge d'une machine à l'autre. Le degré trois
réduit le risque ; il ne l'annule pas.

**Ce document ne dit rien de la sauvegarde D6**, ni du contenu des tests de fumée
D3-09, ni de la forme du plan de mise à jour. Il dit seulement ce qu'un instantané
sait défaire, à quelles conditions on a le droit de le croire, et ce qu'il ne
rattrapera jamais.
