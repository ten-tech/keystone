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
- [ ] **Defender en temps réel** — l'état effectif, distinct de la configuration
- [ ] **Protection DMA** — hors registre, exige l'API Kernel DMA Protection
- [ ] **TPM** : présence, version, état, propriétaire
- [ ] **BitLocker par volume** : état, méthode, protecteurs, *présence de la clé de récupération*
- [ ] Tâches planifiées
- [ ] Règles de pare-feu
- [ ] **Usure NVMe / SMART**, santé et cycles de la batterie
- [ ] Firmware UEFI, microcode
- [ ] **Cohérence de l'horloge** (D1-09) — prérequis de toute la valeur forensique

### 0.3 — Réconciliation d'inventaire logiciel *(le morceau à forte valeur)*

- [x] Lecture des trois vues `Uninstall` du registre — HKLM 64 bits, `WOW6432Node`, HKCU
- [x] Résolution vers une clef de rapprochement canonique
- [x] Détection de Scoop, Chocolatey, JetBrains Toolbox, VS Installer, winget
- [x] Attribution pour Scoop, Chocolatey, JetBrains Toolbox, VS Installer
- [ ] **Attribution winget** — bloquant pour le livrable
- [ ] MSIX et Store, via le dépôt `AppModel` du registre
- [ ] Détection des applications installées et jamais lancées (données d'usage SRUM)

> **État réel, mesuré sur un poste :** 63 applications trouvées, **attribution
> partielle**. winget est détecté mais pas interrogeable — son inventaire vit dans
> une base SQLite (`StoreEdgeFD`) au format non contractuel — donc les applications
> qu'il gère remontent aujourd'hui comme non attribuées.
>
> Le modèle le dit plutôt que de le taire : `inventory.software.attribution` vaut
> « partielle », et `unqueryable_managers` nomme le coupable. Le décompte des non
> attribuées est un **majorant**, pas une mesure — et le pourcentage n'est pas
> publié du tout tant que c'est le cas. Un « 100 % d'orphelines » qui signifie
> « on n'a pas su regarder » serait exactement l'indicateur non explicable que le
> principe P6 interdit.
>
> **Conséquence pour le critère de sortie de la Phase 0 :** il exige « la liste des
> applications gérées par aucun gestionnaire ». Tant que l'attribution est
> partielle, cette liste n'existe pas — seule celle des non attribuées existe.
> L'attribution winget est donc sur le chemin critique, et rusqlite (prévu en 0.5)
> pourrait être avancé pour lire sa base.

> C'est le livrable le plus sous-estimé du produit. Sur un poste réel, la liste des
> orphelins représente typiquement 30 % des applications installées — et c'est
> exactement là que dorment les versions vulnérables depuis quatorze mois.

### 0.4 — WSL et VM en lecture

- [ ] Inventaire des distros : version, noyau, systemd, **taille réelle du `ext4.vhdx`**
- [ ] Inventaire Hyper-V : état, **âge des points de contrôle**, chaînes de disques différentiels
- [ ] Déploiement et exécution de `ks-agent` dans une distro, remontée vers l'hôte

### 0.5 — Journal et rapport

- [ ] **Remplacer le bouchon FNV-1a de `JournalEntry::digest()` par BLAKE3** — première tâche de sécurité réelle du projet
- [ ] Persistance SQLite (WAL)
- [ ] `ks journal` en lecture
- [ ] Rapport HTML autonome, reprenant les tokens de `design/tokens.css`

---

## Phase 1 — Décrire · ~4 semaines

**Objectif :** savoir dire ce qu'on veut, et mesurer l'écart. **Toujours aucune
écriture système.**

**Critère de sortie :** un `workstation.yaml` généré depuis l'état réel, un diff
exact, et la dérive suivie pendant 7 jours sans faux positif inexpliqué.

- [ ] JSON Schema publié et validation stricte (`schema/workstation.schema.json` existe déjà)
- [ ] **Import de l'existant** (D2-02) : génération du yaml initial depuis la machine. *Aucun formulaire à remplir — c'est le moment de vérité M1.*
- [ ] Moteur de diff, par domaine et par item
- [ ] **Attribution de la source du changement** (D2-05) : Windows Update, MDM, installeur, humain, **inconnu**
- [ ] Dérive acceptée avec raison et expiration **obligatoires** (D2-06)
- [ ] Journal des décisions (D2-07)
- [ ] Héritage base + surcouche par machine (D2-10)
- [ ] Intégration git : commit automatique du yaml à chaque changement accepté
- [ ] `ks diff` opérationnel

---

## Phase 2 — Converger · ~6 semaines

**Objectif :** la première écriture. **À partir d'ici, tout se teste dans la VM de
labo** — voir [`06-VM-DE-LABO.md`](06-VM-DE-LABO.md).

**Critère de sortie :** une mise à jour volontairement cassée est détectée par un test
de fumée et annulée automatiquement en moins de 15 minutes, sans intervention (critère A3).

- [ ] Broker : service Windows, gRPC sur named pipe, jeton de session, ACL
- [ ] Contrôle d'intégrité au démarrage, mode sans échec en lecture seule (SEC-07)
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

## Les cinq prochaines tâches, concrètement

Si tu ouvres le projet demain matin :

1. **Remplacer `JournalEntry::digest()`** par BLAKE3. Le bouchon est annoncé dans le
   code, il ne doit pas survivre à la Phase 0.
2. **Collecteur TPM + Secure Boot + BitLocker.** Le socle de tout le domaine D5, et le
   premier vrai contact avec les API Windows.
3. **Réconciliation d'inventaire logiciel.** Le plus gros gain de valeur immédiat, et
   100 % lecture seule.
4. **Persistance SQLite + `ks journal`.** Sans historique, la dérive n'est pas
   mesurable.
5. **Créer la VM de labo** — même si la Phase 2 est loin. La créer maintenant évite de
   se retrouver bloqué trois heures le jour où on en a besoin.

## Ce qui est explicitement refusé

Pour que le projet finisse un jour (risque R6) :

administration de parc au-delà de 5 machines · moteur de détection comportementale ·
remplacement d'une MDM · Windows 10 et éditions Famille · macOS ou Linux comme hôte ·
antivirus · analyse de malware · gestion de licences d'entreprise · toute forme de
« nettoyeur » ou d'« optimiseur ».
