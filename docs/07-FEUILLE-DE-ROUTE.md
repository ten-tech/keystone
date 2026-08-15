# 07 — Feuille de route

## La règle de séquencement, non négociable

**La lecture seule vient d'abord.** Les phases 0 et 1 n'écrivent rien du tout.

Ce n'est pas de la prudence excessive, c'est une condition d'adoption : un outil qui
écrit avant d'avoir prouvé qu'il sait lire ne sera jamais installé sur une machine de
production — y compris la tienne. Et cette séquence a un effet secondaire précieux :
tu obtiens de la valeur utilisable (l'inventaire, la liste des applications non
gérées, le journal) avant d'avoir pris le moindre risque.

---

## Phase 0 — Observer · ~3 semaines

**Objectif :** tout voir, ne rien toucher.

**Critère de sortie :** l'inventaire complet et la liste des applications gérées par
aucun gestionnaire sont produits sur une machine réelle, et l'audit du journal
confirme qu'aucun octet n'a été modifié.

### 0.1 — Socle ✅ fait

- [x] Workspace cargo, 5 crates, CI
- [x] `ks-core` : `Item`, `Drift`, `Plan`, `Action`, `Snapshot`, `JournalEntry`
- [x] Règles P2/P3 tenues par contrôle d'admission testé — pas par le typage, voir `03-ARCHITECTURE.md` § « invariants portés par le modèle »
- [x] `ks-collectors` : trait `Collector`, `Inventory`, collecteur matériel portable
- [x] `ks-cli` : contrat complet des commandes ; `scan`, `status`, `explain` opérationnels
- [x] `ks-broker` : énumération des verbes + barrière de test SEC-02
- [x] `ks-agent-linux` : scan local

### 0.2 — Collecteurs Windows

- [x] **Secure Boot**, VBS/HVCI, Credential Guard, protection LSA — lus par le registre
- [x] **Defender** : exclusions, règles ASR, date des signatures, versions du moteur et des signatures
- [x] Démarrage de six services de sécurité — WinDefend, MpsSvc, EventLog, Sense, wscsvc, BITS. Une liste choisie, pas un inventaire général
- [x] **Pare-feu** : les trois profils, et les règles autorisant une connexion entrante *active* — 197 sur 649 relevées, le total ne se dépliant en rien
- [x] **Firmware et microcode** : fabricant, version, date de publication, révision du microcode en hexadécimal brut
- [x] **Horloge, la configuration** : source de temps et fuseau
- [x] **État effectif** de VBS, de l'intégrité mémoire, de Credential Guard et du temps réel Defender — par WMI, voir [ADR-0005](adr/0005-lecture-detat-effectif.md)
- [ ] **Protection DMA** — seule sa *disponibilité* matérielle est lisible, pas son activation
- [ ] **Usure NVMe / SMART**, santé et cycles de la batterie — `DeviceIoControl`, donc `unsafe`, donc une autre décision
- [ ] **Cohérence de l'horloge** (D1-09) — une mesure contre une référence externe, pas une lecture

> **Trois sujets quittent la Phase 0**, non par manque d'API mais faute de
> privilège. Mesuré en session non élevée, c'est-à-dire dans le contexte où la
> CLI s'exécute réellement (SEC-01) :
>
> - `Win32_Tpm` — **accès refusé** ;
> - `Win32_EncryptableVolume`, donc BitLocker par volume — **accès refusé** ;
> - `Schedule\TaskCache\Tree`, donc les tâches planifiées — **accès refusé**.
>
> Aucun choix de bibliothèque ne les rendra lisibles. Ils appartiennent au
> broker, donc à la **Phase 2**. Les annoncer en Phase 0 aurait été une promesse
> que la plateforme interdit de tenir.

### 0.3 — Réconciliation d'inventaire logiciel *(le morceau à forte valeur)*

- [x] Lecture des trois vues `Uninstall` du registre — HKLM 64 bits, `WOW6432Node`, HKCU
- [x] Résolution vers une clef de rapprochement canonique
- [x] Détection de Scoop, Chocolatey, JetBrains Toolbox, VS Installer, winget
- [x] Attribution pour Scoop, Chocolatey, JetBrains Toolbox, VS Installer
- [x] **Attribution winget** — ses bases de suivi se lisent, une par source
- [x] MSIX et Store, via le dépôt `AppModel` du registre
- [ ] **Canal de service d'un paquet MSIX** — Store ou dépôt manuel, indiscernables au registre
- [ ] Distinguer les applications à mise à jour autonome des vraies orphelines
- [ ] Détection des applications installées et jamais lancées (données d'usage SRUM)

> **État réel, mesuré sur un poste :** 150 applications, dont 10 attribuées,
> **54 non attribuées** et 86 empaquetées. Attribution « complète ».
>
> Deux corrections successives ont amené ce chiffre. La première : winget est
> désormais interrogeable. Ses bases de suivi rangent le code produit, qui est
> exactement le nom de la clé de désinstallation — un rapprochement sûr là où le
> nom échoue, comme `readyfor` qui s'affiche « Smart Connect » au registre et
> « Ready For Assistant » chez winget.
>
> La seconde, plus large : l'inventaire ne voyait **que** le registre. PowerShell 7
> l'a révélé, installé par winget et pourtant introuvable dans les trois vues
> `Uninstall`, parce qu'un paquet MSIX n'en pose aucune. Il manquait 87
> applications sur 150.
>
> Ces 86 paquets restants sont comptés **à part**, et non parmi les orphelines. Un
> paquet MSIX a toujours un canal de service ; le registre ne dit pas lequel. Les
> verser dans les non attribuées ferait passer l'indicateur de 54 à 140 sans qu'un
> seul logiciel de plus soit à l'abandon — un faux positif qui coûterait sa
> crédibilité à l'outil entier.

> C'est le livrable le plus sous-estimé du produit. Sur un poste réel, la liste des
> orphelins représente typiquement 30 % des applications installées — et c'est
> exactement là que dorment les versions vulnérables depuis quatorze mois.

### 0.4 — WSL et VM en lecture

- [x] Inventaire des distros par le registre : version, **taille réelle du `ext4.vhdx`**, intégration et montage des lecteurs
- [ ] Noyau et `systemd` de chaque distro — exige d'y exécuter quelque chose, donc pas un collecteur
- [ ] Inventaire Hyper-V : état, **âge des points de contrôle**, chaînes de disques différentiels
- [ ] Déploiement et exécution de `ks-agent` dans une distro, remontée vers l'hôte

> **Mesuré :** deux distributions, dont un `ext4.vhdx` de **52,2 Gio**. Ce fichier
> grossit et ne se réduit jamais seul : supprimer des données dans la distribution
> ne rend pas un octet à Windows. Invisible depuis l'explorateur, puisqu'il vit
> dans un dossier de paquet — c'est typiquement le premier poste d'occupation d'un
> poste de développement, et personne ne le sait.

### 0.5 — Journal et rapport

- [x] **BLAKE3 remplace le bouchon FNV-1a** (ADR-0004) — la portée exacte, et ses limites, y sont écrites
- [x] Persistance SQLite (WAL), avec `ks scan --record`
- [x] `ks journal` en lecture, avec vérification du chaînage
- [x] Rapport HTML autonome, reprenant les tokens de `design/tokens.css`
- [ ] Expédition du journal vers une ancre externe (`--seal`) — SEC-04, Phase 3

> **Rien n'est consigné par défaut.** `ks scan` ne journalise pas ; il faut
> `ks scan --record`. C'est le principe P2 appliqué à la lettre : puisqu'il
> n'existe pas de `--dry-run` dans ce produit, il ne doit pas non plus exister
> d'écriture par omission. Un scan qui journaliserait sans qu'on l'ait demandé
> contredirait sa propre bannière, et le contredirait en silence.
>
> **La barrière a été éprouvée en la franchissant.** Une entrée réécrite en base,
> puis une entrée effacée : les deux rompent le chaînage, et deux tests le
> rejouent. Une vérification qu'on n'a jamais mise en défaut ne prouve rien.

---

## Phase 1 — Décrire · ~4 semaines

**Objectif :** savoir dire ce qu'on veut, et mesurer l'écart. **Toujours aucune
écriture système.**

**Critère de sortie :** un `workstation.yaml` généré depuis l'état réel, un diff
exact, et la dérive suivie pendant 7 jours sans faux positif inexpliqué.

- [ ] JSON Schema publié et validation stricte (`schema/workstation.schema.json` existe déjà)
- [x] **Import de l'existant** (D2-02) : `ks import` écrit le yaml depuis l'état lu de la machine, par un émetteur maison qui guillemète tout texte et n'écrit jamais ce qu'il n'a pas su lire (ADR-0016). *Aucun formulaire à remplir — c'est le moment de vérité M1.*
- [x] Moteur de diff, par domaine et par item : `ks diff` publie les quatre verdicts, l'écart et l'incomparable ligne à ligne (ADR-0008)
- [ ] **Attribution de la source du changement** (D2-05) : Windows Update, MDM, installeur, humain, **inconnu**
- [ ] Dérive acceptée avec raison et expiration **obligatoires** (D2-06)
- [ ] Journal des décisions (D2-07)
- [ ] Héritage base + surcouche par machine (D2-10)
- [x] Le yaml est écrit atomiquement, sans écrasement implicite ; la commande git est **proposée**, jamais exécutée (ADR-0017)
- [x] `ks diff` opérationnel

---

## Phase 2 — Converger · ~6 semaines

**Objectif :** la première écriture. **À partir d'ici, tout se teste dans la VM de
labo** — voir [`06-VM-DE-LABO.md`](06-VM-DE-LABO.md).

**Critère de sortie :** une mise à jour volontairement cassée est détectée par un test
de fumée et annulée automatiquement en moins de 15 minutes, sans intervention (critère A3).

- [ ] Broker : service Windows, gRPC sur named pipe, jeton de session, ACL
- [ ] Contrôle d'intégrité au démarrage, mode sans échec en lecture seule (SEC-07)
- [ ] **Ce que la Phase 0 n'a pas pu lire, faute de privilège** — mesuré en accès refusé sans élévation, donc reporté ici et non abandonné :
  - **TPM** : présence, version, état, propriétaire (`Win32_Tpm`)
  - **BitLocker par volume** : état, méthode, protecteurs, *présence de la clé de récupération* (`Win32_EncryptableVolume`)
  - **Tâches planifiées** (`Schedule\TaskCache\Tree`)
  - Ces lectures restent des **lectures** : elles n'ajoutent aucun verbe, et ne relèvent donc pas des quatre questions du §7
- [ ] Moteur d'instantanés : point de restauration, checkpoint Hyper-V, `wsl --export`, export de registre
- [ ] Convergence par item et par domaine, **simulation obligatoire d'abord**
- [ ] Tests d'idempotence automatisés (D2-09, critère A2)
- [ ] Plan de mise à jour multi-OS, anneaux, vagues ordonnées, dépendances
- [ ] **Tests de fumée définis par l'utilisateur** (D3-09)
- [ ] **Rollback automatique + épinglage du fautif + compte rendu** (D3-10) — moment M5
- [ ] Regroupement des redémarrages (D3-05)
- [ ] **Domaine D6 — sauvegarde** : moteur Restic/Kopia, 3-2-1, **test de restauration automatique** (critère A7), alerte de travail non poussé
- [ ] Windows Hello sur les verbes coûteux (SEC-08)

---

## Phase 3 — Tenir · ~6 semaines

**Critère de sortie :** un changement de posture injecté est détecté en moins de
5 minutes et visible dans le journal externe (critère A4).

- [ ] **Ancres externes** : expédition du journal en écriture seule, battement de cœur, référence PCR (SEC-04/05/06)
- [ ] D4 — attribution de l'espace par consommateur, quarantaine avec durée de vie (critère A6)
- [ ] D5 — diffs de persistance, **magasin de certificats racine**, ports en écoute, référence de trafic sortant
- [ ] D5-08 — canaris : fichiers leurres, clé cloud piège, compte pot de miel
- [ ] D5-09 — recherche de secrets et crochets git
- [ ] **D14-06 — isolement d'urgence** avec capture forensique et scellement du journal
- [ ] Matrice de couverture ATT&CK, avec les trous affichés (D5-12)

---

## Phase 4 — Vivre · ~6 semaines

**Critère de sortie :** une semaine d'usage réel sans intervention manuelle.

- [ ] D8 — profils contextuels, bascule automatique, gestion de la charge de batterie
- [ ] D9 — distro de référence et clonage, snapshot par distro, compaction du `vhdx`, diagnostic réseau WSL
- [ ] D10 — profils réseau, évaluation du réseau joint, **restauration de la disposition des écrans par dock**, liste blanche USB
- [ ] D11 — docteur d'environnement de dev, exclusions Defender ciblées et expliquées, vue transverse des dépôts git
- [ ] **D12 — coexistence MDM** : détection d'Intune/GPO, conflits, oscillation (critère A10)
- [ ] Budget d'alertes : au plus 2 interruptions par jour (D16-06, critère A15)

---

## Phase 5 — Voir · ~8 semaines

**Critère de sortie :** les 8 moments de vérité du brief sont implémentés et éprouvés
auprès de 5 utilisateurs.

- [ ] Interface Tauri, les 7 écrans du brief §7
- [ ] Health Ring, respiration 0,25 Hz, **ralentissement en cas de dégradation**
- [ ] `prefers-reduced-motion`, `forced-colors`, WCAG 2.2 AA (critère A13)
- [ ] Palette de commandes avec parité CLI affichée
- [ ] D14 — timeline forensique, corrélation des incidents, triage des minidumps
- [ ] D16 — rapport matinal, exports SBOM/OSCAL/SARIF
- [ ] D15 — copilote local, **lecture seule, aucune application automatique**

---

## Phase 6 — Essaimer · ~4 semaines

**Critère de sortie :** un tiers installe Keystone à partir de la seule documentation.

- [ ] D13 — flotte personnelle, promotion par demande de fusion, détection de divergence
- [ ] D16-05 — vue mobile en lecture seule, action unique : isoler
- [ ] Empaquetage signé, amorceur immuable (NF-08)
- [ ] `ks uninstall` propre et vérifié (critère A14)
- [ ] Documentation utilisateur, avec les limites du §4.3 en première page

---

## Les prochaines tâches, concrètement

Si tu ouvres le projet demain matin.

> Cette liste a été périmée sur trois points sur cinq pendant plusieurs
> semaines : elle réclamait BLAKE3 et la persistance SQLite, tous deux livrés
> et cochés en Phase 0.5 quinze lignes plus haut, et rangeait le TPM et
> BitLocker dans « la prochaine tâche » alors que ce même document les renvoie
> explicitement au broker, donc à la Phase 2. Une liste de tâches ne se relit
> pas toute seule : elle se corrige dans le commit qui en accomplit une.

1. **Persister les valeurs observées.** La dérive sur sept jours — critère de
   sortie de la Phase 1 — n'est pas mesurable tant qu'aucun relevé n'est
   comparé au précédent. Le magasin existe, il ne stocke que le journal.
2. **Brancher la coque sur l'état désiré.** `ks import` et `ks diff` existent
   (ADR-0016, ADR-0017), et le chargement vit dans la **bibliothèque** `ks-cli`
   — `confrontation::charger` puis `confrontation::confronter` — précisément
   pour que `ui/ks-ui` s'en serve. Tant qu'elle ne les appelle pas, tous ses
   items lui arrivent sans désir, `Item::verdict()` répond `NonContraint`
   partout, la posture reste « pas encore calculable » et la vue Dérive affiche
   « sans objet ». C'est un branchement, plus une écriture de moteur.
   Reste aussi la **génération du schéma JSON** depuis les types Rust, décidée
   par l'ADR-0010 et non faite : `schema/workstation.schema.json` décrit encore
   la forme imbriquée, que plus rien ne lit.
3. **Réconciliation d'inventaire logiciel** — livrée pour l'essentiel, mais
   l'attribution reste incomplète : 54 applications sur 150 sans gestionnaire
   identifié, et le module le déclare lui-même plutôt que d'arrondir.
4. **Créer la VM de labo** — même si la Phase 2 est loin. La créer maintenant
   évite de se retrouver bloqué trois heures le jour où on en a besoin. C'est
   aussi la seule façon d'éprouver un jour le broker sans risquer l'hôte.
5. **Le TPM, BitLocker et les tâches planifiées** ne sont pas des tâches de
   Phase 0 : ils sont refusés sans élévation, donc ils appartiennent au
   broker. Ils figurent ici pour qu'on cesse de les y chercher.

## Ce qui est explicitement refusé

Pour que le projet finisse un jour (risque R6) :

administration de parc au-delà de 5 machines · moteur de détection comportementale ·
remplacement d'une MDM · Windows 10 et éditions Famille · macOS ou Linux comme hôte ·
antivirus · analyse de malware · gestion de licences d'entreprise · toute forme de
« nettoyeur » ou d'« optimiseur ».
