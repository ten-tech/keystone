# KEYSTONE — Cahier des charges
### Plan de contrôle déclaratif pour poste de travail d'ingénieur

| | |
|---|---|
| **Version** | 1.0 |
| **Date** | 30 juillet 2026 |
| **Statut** | Pour revue |
| **Cible** | Poste Windows 11 24H2+ avec WSL2 et Hyper-V, usage ingénierie |
| **Document jumeau** | [`02-BRIEF-DESIGN.md`](02-BRIEF-DESIGN.md) (direction UI/UX « Poste de pilotage ») |

---

## Sommaire

1. [Contexte et problème](#1-contexte-et-problème)
2. [Vision et thèse produit](#2-vision-et-thèse-produit)
3. [Principes directeurs](#3-principes-directeurs)
4. [Périmètre](#4-périmètre)
5. [Architecture](#5-architecture)
6. [Modèle de sécurité de l'outil lui-même](#6-modèle-de-sécurité-de-loutil-lui-même)
7. [Exigences fonctionnelles](#7-exigences-fonctionnelles)
8. [Exigences non fonctionnelles](#8-exigences-non-fonctionnelles)
9. [Modèle de données](#9-modèle-de-données)
10. [Interfaces](#10-interfaces)
11. [Roadmap et jalons](#11-roadmap-et-jalons)
12. [Critères d'acceptation](#12-critères-dacceptation)
13. [Risques et mitigations](#13-risques-et-mitigations)
14. [Plan de sortie](#14-plan-de-sortie)
15. [Glossaire](#15-glossaire)

---

## 1. Contexte et problème

Un poste d'ingénieur logiciel moderne agrège une complexité que rien n'administre de bout en bout :

- **~10 gestionnaires de mise à jour concurrents** (Windows Update, pilotes, firmware UEFI, MSU/MSIX, Store, winget, Scoop, VS Installer, JetBrains Toolbox) auxquels s'ajoutent les gestionnaires de paquets de chaque distribution WSL2 et les chaînes d'outils globales (cargo, npm, pip, go), sans oublier les applications qui se mettent à jour d'elles-mêmes.
- **Trois à cinq systèmes d'exploitation vivants simultanément** sur la même machine (hôte Windows, N distributions WSL2, M VM Hyper-V), sans vue consolidée.
- **Une dérive de configuration continue et sans auteur identifié** : politiques poussées par la MDM d'entreprise, mises à jour qui réactivent une option, tweaks manuels oubliés depuis six mois.
- **Une surface de sécurité maximale** : droits administrateur permanents, dizaines de chaînes d'outils, exécution de code téléchargé au quotidien.
- **Une saturation d'espace disque opaque** : nul ne sait dire, sans investigation manuelle, ce qui consomme 300 Go.

Les outils constructeurs existants (Lenovo Vantage, Dell SupportAssist, HP Support Assistant, MyASUS) répondent à une fraction de ce besoin sur un modèle inadapté : **des actions ponctuelles, opaques, non réversibles, déclenchées par des boutons**, souvent adossées à un compte cloud et à de la télémétrie, parfois assorties de « nettoyeurs » sans fondement technique. Ils ne connaissent ni WSL, ni les VM, ni les chaînes d'outils de développement.

**Le problème n'est pas l'absence d'outils. C'est l'absence de modèle.**

## 2. Vision et thèse produit

Keystone remplace le modèle « panneau de boutons » par un **modèle déclaratif à convergence** : l'utilisateur décrit l'état souhaité de sa machine dans un fichier versionné, Keystone mesure l'écart en continu, propose une convergence simulée, l'applique sous filet de sécurité, et journalise tout de manière inaltérable.

> **Thèse : une machine d'ingénieur doit être un artefact reproductible, pas un objet unique et fragile.**

**Thèse UX jumelle, qui gouverne tout le design (voir le brief) :**

> **Le calme est la fonctionnalité.** La récompense offerte à l'utilisateur est un écran silencieux et la certitude que si quelque chose bougeait, il le saurait.

**Objectifs mesurables à 12 mois :**

| Indicateur | Départ | Cible |
|---|---|---|
| Temps de maintenance manuelle | ~4 h / mois | < 30 min / mois |
| MTTR après une mise à jour défaillante | 2 à 6 h | < 15 min (rollback automatique) |
| Temps de reconstruction complète du poste | 2 à 3 jours | < 4 h non assistées |
| Écarts de configuration non justifiés | inconnu | 0 en régime permanent |
| Délai de détection d'un changement de posture sécurité | jamais | < 5 min |
| Espace disque libre | dérive continue | stable à ± 5 % sur 6 mois |

## 3. Principes directeurs

Ces principes arbitrent tous les conflits de conception. En cas de contradiction avec une exigence, le principe l'emporte et l'exigence est réécrite.

**P1 — État désiré, pas action ponctuelle.** La source de vérité est un fichier déclaratif versionné. Toute action de l'outil est la conséquence d'un écart mesuré.

**P2 — Simulation par défaut.** Toute opération produit d'abord un diff. `--apply` est explicite et jamais présélectionné.

**P3 — Rien d'irréversible sans filet.** Si Keystone ne sait pas fabriquer le retour arrière d'une opération, il ne l'automatise pas. *Corollaire strict : la réversibilité est un critère d'admission d'une fonctionnalité, pas une option.*

**P4 — Sans interface graphique d'abord.** Le cœur est un service, une CLI et une API locale. L'interface graphique est un client parmi d'autres. Parité fonctionnelle CLI / UI obligatoire.

**P5 — Local, point final.** Aucun compte requis, aucune donnée sortante par défaut, aucune télémétrie éditeur. Les exports sont explicites et à destination choisie par l'utilisateur.

**P6 — Explicabilité.** Chaque item de configuration porte : à quoi il sert, quel risque on prend en le changeant, sa source de référence. Un indicateur composite est toujours dépliable en ses composantes exactes. *Un chiffre inexplicable est une décoration.*

**P7 — Ne jamais empêcher de travailler.** Keystone en panne, dégradé ou en cours de mise à jour ne bloque jamais l'usage de la machine. Il échoue en mode ouvert (*fail-open*) sur ses fonctions de confort, en mode fermé (*fail-closed*) sur ses fonctions de sécurité.

**P8 — Désinstallable proprement.** Une commande retire Keystone et laisse la machine dans un état connu et documenté. Aucun résidu, aucune politique orpheline.

**P9 — Pas de charlatanisme.** Aucun « nettoyeur de registre », aucun « accélérateur de RAM », aucun score marketing. Chaque action doit avoir un effet mesurable et une source citable.

**P10 — Cohabitation, pas conflit.** Sur un poste géré par l'entreprise, la MDM est souveraine. Keystone détecte et signale les conflits ; il ne les gagne pas de force.

## 4. Périmètre

### 4.1 Inclus
Hôte Windows 11 (24H2 et ultérieur, éditions Pro / Enterprise) · distributions WSL2 · VM Hyper-V locales · pilotes et firmware · chaînes d'outils de développement · sauvegarde et restauration des données de travail · posture de sécurité locale · détection de changement · espace disque · réseau, docks et périphériques · flotte personnelle de 1 à 5 machines.

### 4.2 Exclu
Administration de serveurs ou de parc au-delà de la flotte personnelle · remplacement d'un EDR (Keystone est complémentaire, voir D5) · remplacement d'une MDM d'entreprise · Windows 10 et éditions Famille · macOS et Linux comme systèmes hôtes (uniquement comme invités) · antivirus, moteur de détection comportementale, analyse de malware · gestion de licences d'entreprise.

### 4.3 Hors d'atteinte, assumé et documenté
Ces limites sont **structurelles** et doivent figurer dans la documentation utilisateur, sans euphémisme :

- Rootkit noyau ou firmware (UEFI, option ROM, BMC, firmware SSD/NIC) : vit **sous** l'agent. Seule une incohérence peut être constatée, via l'attestation TPM (D5).
- Attaque purement en mémoire ne laissant aucun artefact persistant.
- Vol d'identifiants par canal légitime (jeton OAuth, cookie de session exfiltré depuis un navigateur).
- Hyperviseur ou hôte compromis si la machine est elle-même virtualisée.
- Attaque physique : DMA, *cold boot*, *evil maid* sur machine éteinte.
- Compromission de Keystone lui-même (traitée en §6 et §13).

> **Aucune communication produit ne prétendra « détecter toute intrusion ».** Cette prétention est fausse et détruit la confiance de l'utilisateur expert.

## 5. Architecture

### 5.1 Vue d'ensemble

```
┌─────────────────────────────────────────────────────────────────┐
│  Clients (non privilégiés)                                      │
│  UI Tauri/WebView2   ·   CLI ks   ·   API locale gRPC            │
└──────────────────────────┬──────────────────────────────────────┘
                           │  named pipe + gRPC, jeton par session
┌──────────────────────────▼──────────────────────────────────────┐
│  BROKER (service Windows, Rust, signé, minimal)                 │
│  · API de verbes typés et énumérés — jamais d'exécution libre    │
│  · Autorisation, audit, limitation de débit, contrôle d'intégrité│
│  · Moteur de convergence · Ordonnanceur · Gestion des snapshots  │
└───┬─────────────┬──────────────┬───────────────┬────────────────┘
    │             │              │               │
┌───▼──────┐ ┌────▼──────┐ ┌─────▼──────┐ ┌──────▼───────────────┐
│Collecteurs│ │Exécuteurs │ │  Agents    │ │ Persistance          │
│(WASM      │ │(PowerShell│ │ satellites │ │ SQLite (état+hist.)  │
│ bac à     │ │ 7, CIM,   │ │ WSL: ELF   │ │ Journal append-only  │
│ sable)    │ │ WinAPI)   │ │ VM: hvsock │ │ Exporteur Prometheus │
└───────────┘ └───────────┘ └────────────┘ └──────────────────────┘
                                              │
                          ┌───────────────────▼──────────────────┐
                          │ Ancres externes (hors machine)       │
                          │ · Journal expédié en écriture seule  │
                          │ · Référence d'attestation TPM (PCR)  │
                          │ · Dépôt git de workstation.yaml      │
                          └──────────────────────────────────────┘
```

### 5.2 Choix technologiques et justification

| Composant | Choix | Justification |
|---|---|---|
| Broker privilégié | **Rust**, service Windows | Pas de runtime à charger, empreinte minimale, binaire facile à signer et à attester, sûreté mémoire sur le composant qui a le plus de privilèges. |
| Transport | **gRPC sur named pipe** | Contrat typé, pas de port TCP exposé, ACL Windows natives sur le pipe. |
| Collecteurs | **natif pour les nôtres, WASM (wasmtime) pour les tiers** | Deux régimes selon l'origine — voir [ADR-0003](adr/0003-collecteurs-en-wasm.md). Nos collecteurs ont besoin d'un accès direct à WMI et au registre ; tout mettre en WASM imposerait une API hôte si large qu'elle deviendrait elle-même la surface d'attaque. Un collecteur tiers, lui, ne doit jamais hériter du privilège du broker : bac à sable, capacités déclarées, plantage isolé. |
| Exécuteurs Windows | **PowerShell 7 + CIM/WMI + WinAPI** | BitLocker, Defender, MSU, politiques : PowerShell est la seule voie de première classe. Appelé par le broker uniquement, jamais exposé brut. |
| Agent WSL | **binaire ELF statique** sous systemd | Mêmes protocole et schéma que l'hôte. |
| Canal VM | **Hyper-V Socket (AF_HYPERV)** | Fonctionne sans réseau dans l'invité — indispensable quand la VM est justement cassée au niveau réseau. |
| Persistance | **SQLite** (WAL) + journal *append-only* | Local, sans service, requêtable, sauvegardable. |
| Interface | **Tauri / WebView2** | Un seul binaire, empreinte mémoire faible, la même base de code que la future vue mobile en lecture seule. |
| Métriques | **exporteur Prometheus** optionnel | Interopérabilité sans imposer de pile. |

### 5.3 Modèle de plugin

Chaque capacité est un module déclarant dans son manifeste : `reads` · `writes` · `privileges` · `supports_dryrun` · `supports_rollback` · `idempotent` · `platform` · `signature`.

**Un module sans `supports_dryrun` ne peut pas être appelé en mode `apply` (P2). Un module sans `supports_rollback` ne peut pas être appelé par l'ordonnanceur automatique (P3).** Ces deux règles sont vérifiées par le broker au chargement, pas par convention.

**Un collecteur déclare `writes: []`.** Un module qui déclare une écriture n'est pas un collecteur, et le chargement le refuse.

> **État d'implémentation.** Le manifeste n'existe pas encore en code : seule la
> structure `Capabilities` de `ks-core` en porte trois champs (`dry_run`,
> `rollback`, `idempotent`), et le broker n'a pas de chargeur de module. Tant
> que c'est le cas, les deux règles ci-dessus tiennent par la discipline de
> l'auteur, pas par une vérification. Le passage de l'une à l'autre est une
> tâche de la Phase 2, et elle se paie d'autant plus cher qu'on l'ajourne.

## 6. Modèle de sécurité de l'outil lui-même

> Keystone administre tout, avec les droits les plus élevés. **Compromis, il constitue l'outil d'attaque le plus efficace imaginable sur ce poste.** Ce chapitre n'est pas une annexe : c'est une condition d'existence du projet.

| Réf. | Exigence |
|---|---|
| **SEC-01** | Séparation de privilèges : l'interface et la CLI sont non privilégiées. Seul le broker est élevé. |
| **SEC-02** | L'API du broker expose des **verbes typés et énumérés**. Aucune primitive d'exécution arbitraire de commande ou de script n'est exposée, en aucune circonstance. |
| **SEC-03** | Toute action est journalisée dans un journal *append-only* : horodatage monotone, identité appelante, verbe, paramètres, diff simulé, résultat. |
| **SEC-04** | Le journal est **expédié hors machine en écriture seule** (destination *append-only*). Un attaquant qui a SYSTEM peut effacer le local, pas ce qui est déjà parti. |
| **SEC-05** | Un **battement de cœur** est attendu côté puits externe. **L'absence de battement est elle-même une alerte** — le silence de l'agent est un signal, pas une absence de signal. |
| **SEC-06** | **Attestation TPM** : les PCR du Measured Boot et du DRTM sont comparés à une référence conservée hors machine. Une divergence est remontée même si aucun artefact n'est visible. |
| **SEC-07** | Binaires et modules **signés**. Le broker refuse tout module non signé et vérifie sa propre intégrité au démarrage ; en cas d'échec il démarre en mode lecture seule et alerte. |
| **SEC-08** | **Confirmation Windows Hello** obligatoire sur les verbes à coût réel : désactiver une protection, modifier WDAC, suspendre BitLocker, purger une quarantaine, isoler la machine, mettre à jour le firmware. |
| **SEC-09** | Secrets scellés au TPM (DPAPI-NG). Aucun secret en clair sur disque, aucun secret dans les journaux — **rédaction obligatoire à l'écriture**, pas au moment de l'affichage. |
| **SEC-10** | Limitation de débit et fenêtre de refroidissement sur les verbes destructeurs. Une rafale d'actions destructrices est bloquée et alertée, y compris si elle est légitimement demandée. |
| **SEC-11** | Pas d'auto-mise-à-jour silencieuse de Keystone. Voir NF-08 pour le problème de l'amorçage. |
| **SEC-12** | Le broker n'écoute sur **aucun port réseau**. La consultation à distance passe par un relais sortant explicitement activé. |
| **SEC-13** | Modèle de menace documenté et **revu à chaque version majeure**, avec la liste des limites du §4.3 tenue à jour. |

## 7. Exigences fonctionnelles

Priorités : **P0** = indispensable au premier usage réel · **P1** = valeur forte, après P0 · **P2** = différenciant, non bloquant.

---

### D1 — Inventaire et santé *(P0)*

| Réf. | Exigence |
|---|---|
| D1-01 | Inventaire matériel : CPU, RAM (canaux, fréquence), stockage avec **usure et heures de service NVMe/SMART**, GPU, batterie avec **santé et nombre de cycles**, écrans, docks, périphériques USB. |
| D1-02 | Inventaire firmware : version UEFI, microcode, firmware SSD/NIC, avec comparaison au flux constructeur. |
| D1-03 | Posture plateforme : TPM (version, état, propriétaire), Secure Boot, VBS/HVCI, Credential Guard, LSA protection, DMA protection. |
| D1-04 | Chiffrement : état BitLocker **par volume**, méthode, protecteurs, **présence et localisation de la clé de récupération**. |
| D1-05 | Inventaire logiciel unifié — voir D3-01 (réconciliation). |
| D1-06 | Inventaire WSL par distribution : version WSL, noyau, systemd, **taille réelle et taille allouée de `ext4.vhdx`**, paquets en retard, points de montage. |
| D1-07 | Inventaire VM : état, points de contrôle avec **âge et chaîne de disques différentiels**, virtualisation imbriquée, partitionnement GPU. |
| D1-08 | Santé thermique et énergétique : températures, plafonnements (*throttling*), consommation instantanée en watts, profil d'alimentation actif. |
| D1-09 | **Cohérence de l'horloge** : dérive NTP, source de temps, décalage entre hôte, distros et VM. *Prérequis absolu de la valeur forensique du journal (D14).* |
| D1-10 | Tout item d'inventaire porte sa **date de dernière observation** et sa **source de collecte**. Une donnée sans provenance n'est pas affichée. |

### D2 — État désiré et dérive *(P0)* — cœur du produit

| Réf. | Exigence |
|---|---|
| D2-01 | Source de vérité unique : `workstation.yaml`, versionné en git, schéma JSON Schema publié et validé. |
| D2-02 | **Import de l'existant au premier lancement** : Keystone génère le `workstation.yaml` initial à partir de l'état réel de la machine. **Aucun formulaire à remplir.** *(Moment M1 du brief — condition de l'adoption.)* |
| D2-03 | Domaines couverts : services, tâches planifiées, règles de pare-feu, valeurs de registre, politiques locales, profils d'alimentation, applications et versions, distros WSL et leurs jeux de paquets, VM, fichiers de configuration (*dotfiles*), variables d'environnement, associations de fichiers. |
| D2-04 | Détection de dérive **continue** (événementielle quand la source le permet, sinon sondage) avec un diff lisible par un humain. |
| D2-05 | **Attribution de la source du changement** : Windows Update, MDM/Intune, installeur applicatif, utilisateur (avec horodatage), **inconnu**. Une dérive sans auteur identifié est marquée comme signal de sécurité et remontée à D5. |
| D2-06 | **Dérive acceptée** : un écart peut être accepté, avec **raison obligatoire** et **date d'expiration obligatoire**. À l'expiration, l'item redevient une dérive active. Aucune exception permanente silencieuse. |
| D2-07 | **Journal des décisions** : chaque acceptation, chaque épinglage, chaque exclusion produit une entrée durable et consultable expliquant le pourquoi. *L'oubli du pourquoi est la principale cause de pourrissement des configurations.* |
| D2-08 | Convergence sélective : par item, par domaine, ou totale. Toujours simulée d'abord (P2). |
| D2-09 | **Idempotence garantie** : appliquer deux fois produit le même état, et la seconde exécution ne rapporte aucun changement. Vérifié par test automatisé (NF-07). |
| D2-10 | Héritage de configuration : un fichier de base commun + une surcouche par machine, pour la flotte personnelle (D13). |

### D3 — Mises à jour orchestrées *(P0)*

| Réf. | Exigence |
|---|---|
| D3-01 | **Réconciliation d'inventaire** : croiser clés `Uninstall`, MSIX, Store, winget, Scoop, Chocolatey, VS Installer, JetBrains Toolbox et installeurs propriétaires vers un identifiant canonique, puis **produire la liste des applications gérées par aucun gestionnaire**. *Livrable le plus sous-estimé du produit : c'est là que se trouvent les versions vulnérables depuis des mois.* |
| D3-02 | Politique par application déclarée dans le yaml : anneau (`canary` / `stable` / `manuel`), épinglage avec **raison et date d'expiration**, fenêtre de maintenance. |
| D3-03 | **Couloir sécurité** : une vulnérabilité critique et activement exploitée court-circuite l'anneau, avec notification explicite du contournement. |
| D3-04 | Plan de mise à jour unifié, tous systèmes confondus, présenté avant toute action : diff, journal des modifications récupéré, indication de vulnérabilité, redémarrage requis. |
| D3-05 | **Regroupement des redémarrages** : un seul redémarrage pour l'ensemble d'un plan. |
| D3-06 | Fenêtre d'application respectant le contexte réel : pas de build en cours, pas de réunion à l'agenda, pas de présentation active, pas sur batterie sous 40 %, pas sur réseau facturé au volume. |
| D3-07 | **Instantané avant application** : point de contrôle Hyper-V, `wsl --export`, ou point de restauration selon la cible. Une cible sans instantané possible n'est pas éligible à l'application automatique (P3). |
| D3-08 | **Application par vagues ordonnées**, avec dépendances déclarées. Deux composants couplés (noyau WSL et Docker Desktop, par exemple) ne sont jamais mis à jour dans la même transaction : sinon l'imputation de la panne est impossible. |
| D3-09 | **Tests de fumée définis par l'utilisateur** comme unique juge du résultat : build d'un dépôt de référence, `docker run`, montée du VPN, détection GPU par CUDA, démarrage de l'IDE. |
| D3-10 | **Rollback automatique** en cas d'échec d'un test, avec épinglage automatique du composant fautif et compte rendu de ce qui a été annulé. *(Moment M5 du brief.)* |
| D3-11 | Chaînes d'outils de développement : Keystone met à jour le **lanceur de versions** (mise, asdf, volta), jamais les *runtimes* épinglés par projet — la reproductibilité des projets prime. Il signale en revanche les *runtimes* en fin de support. |
| D3-12 | Pilotes et firmware : **jamais via Windows Update**, toujours via le flux constructeur avec validation explicite. Le firmware exige secteur branché, batterie > 50 %, suspension propre de BitLocker, et n'est jamais automatique. |
| D3-13 | Applications à mise à jour autonome (navigateurs, IDE, clients de messagerie) : **observées, non combattues**. Chaque changement de version détecté est écrit dans la timeline (D14) pour la traçabilité. |
| D3-14 | Cache local de paquets et mode hors ligne : rien ne se déclenche sur connexion facturée au volume. |
| D3-15 | Distros WSL et VM : mêmes anneaux, mais **automatisation plus poussée assumée**, l'instantané y étant instantané et le retour arrière de l'ordre de 20 secondes. |

### D4 — Espace et propreté *(P0)*

| Réf. | Exigence |
|---|---|
| D4-01 | **Attribution**, pas « nettoyage » : répondre à « qu'est-ce qui consomme mon disque » avec une ventilation par consommateur — WinSxS, `vhdx` gonflés (Docker, WSL), `node_modules` abandonnés, caches cargo/nuget/pip/npm, `Windows.old`, vidages sur incident, caches de shaders, MSIX orphelins. |
| D4-02 | Détection d'applications **installées et jamais lancées** (données d'usage SRUM), avec date de dernière utilisation. |
| D4-03 | Actions de récupération sûres : `fstrim` puis compaction de `vhdx`, purge Docker avec politique d'âge, ramasse-miettes par chaîne d'outils, purge des points de restauration au-delà de N. |
| D4-04 | **Quarantaine, jamais suppression** : tout élément récupéré part dans une zone de quarantaine avec durée de vie (30 jours par défaut) et restauration en un geste. La purge définitive exige Windows Hello (SEC-08). |
| D4-05 | Historique de l'espace libre sur 24 mois, avec projection de saturation. |
| D4-06 | Détection des consommateurs à croissance anormale (un cache qui double en une semaine est signalé avant la saturation, pas après). |

### D5 — Posture de sécurité et détection de changement *(P0)*

> **Positionnement explicite : Keystone n'est pas un EDR.** L'EDR traque le comportement malveillant ; Keystone garantit que **la posture de l'EDR n'a pas été sabotée** et que rien n'a changé sans que l'utilisateur le sache. Beaucoup d'intrusions réelles commencent par « quelqu'un a désactivé la protection en temps réel » — c'est précisément ce que Keystone voit en moins de cinq minutes.

| Réf. | Exigence |
|---|---|
| D5-01 | Score de posture adossé au sous-ensemble utile des référentiels Microsoft Security Baseline et CIS, **entièrement dépliable en ses composantes** (P6). |
| D5-02 | **Diff de persistance** : clés Run, services, tâches planifiées, abonnements aux événements WMI, détournements COM, extensions de shell. Toute nouveauté est remontée. |
| D5-03 | **Diff du magasin de certificats** — détection d'une autorité racine ajoutée. *Signal à très haute valeur : c'est la signature de l'interception TLS.* |
| D5-04 | Diff des ports en écoute et **référence de trafic sortant par processus** (ETW / WFP) : une destination hors référence est signalée. |
| D5-05 | Contrôle de signature de tout binaire exécuté au démarrage ; éditeur inconnu signalé. |
| D5-06 | Surveillance de la posture des protections : Defender (temps réel, exclusions, règles ASR), pare-feu, LSA protection, HVCI, BitLocker, WDAC. Toute désactivation est une alerte immédiate. |
| D5-07 | Hygiène des comptes : création de compte local, ajout au groupe Administrateurs, attribution de privilège sensible, session administrateur permanente. |
| D5-08 | **Canaris** : fichiers leurres dans les répertoires de développement, identifiants pièges (clé cloud factice dont tout usage déclenche une alerte), compte pot de miel. *Détecte l'adversaire par son comportement, y compris un adversaire inconnu.* |
| D5-09 | Recherche de secrets dans les répertoires de travail et installation de crochets git de pré-commit. |
| D5-10 | Attestation TPM contre référence externe (SEC-06). |
| D5-11 | Consommation de règles **Sigma** et intégration Sysmon optionnelle, sans embarquer de moteur de détection propre. |
| D5-12 | **Documentation honnête des limites** : la couverture est présentée sous forme de matrice face à MITRE ATT&CK, avec les zones non couvertes affichées explicitement (§4.3). |

### D6 — Sauvegarde, restauration et reconstruction *(P0)*

> **Angle mort le plus grave d'une première conception centrée sur la configuration :** des instantanés de rollback ne sont pas une sauvegarde. Ils protègent la machine, pas le travail.

| Réf. | Exigence |
|---|---|
| D6-01 | Sauvegarde des **données de travail** (dépôts, documents, machines virtuelles, clés) selon une règle **3-2-1** : trois copies, deux supports, une hors site. |
| D6-02 | Moteur de sauvegarde déduplicant et chiffré (Restic ou Kopia), clé scellée au TPM, avec dépôt hors site chiffré côté client. |
| D6-03 | **Test de restauration automatique et périodique** : Keystone restaure un échantillon aléatoire dans un espace temporaire et vérifie les empreintes. **Une sauvegarde jamais restaurée n'est pas une sauvegarde.** |
| D6-04 | **Alerte de travail non sauvegardé** : détection des dépôts avec des modifications non validées ou des validations non poussées depuis plus de N heures. *Cause n°1 réelle de perte de données sur un poste d'ingénieur.* |
| D6-05 | Instantanés de rollback (D3-07) clairement distingués des sauvegardes dans l'interface et la documentation. |
| D6-06 | **Amorçage de reconstruction** : génération d'un script de reconstruction complète depuis le dépôt git — `autounattend.xml`, import winget, import des distros, restauration des données, remontage des VM. |
| D6-07 | Objectifs déclarés et vérifiés : **RPO ≤ 1 h** pour les données de travail, **RTO ≤ 4 h** pour un poste complet. |
| D6-08 | Répétition de sinistre : une commande éprouve la reconstruction dans une VM éphémère et rapporte le temps réel obtenu. *(Moment M8 du brief.)* |

### D7 — Secrets et identités *(P1)*

| Réf. | Exigence |
|---|---|
| D7-01 | Inventaire des identités et matériels cryptographiques : clés SSH, clés GPG, certificats client, clés FIDO2, jetons d'API, chaînes de connexion. |
| D7-02 | **Suivi d'expiration** avec préavis : certificats, jetons, clés à durée de vie, abonnements. |
| D7-03 | Détection des secrets stockés en clair (fichiers `.env`, historiques de shell, fichiers de configuration) avec proposition de migration vers un coffre. |
| D7-04 | Assistance à la rotation : procédure guidée et liste des emplacements à mettre à jour. |
| D7-05 | Inventaire des **codes de récupération** et vérification de leur mise à l'abri (l'absence de code de récupération sauvegardé est une alerte). |
| D7-06 | Vérification de l'absence de secret dans les journaux de Keystone lui-même (SEC-09), par test automatisé. |

### D8 — Profils contextuels, énergie et thermique *(P1)*

| Réf. | Exigence |
|---|---|
| D8-01 | Profils nommés modifiant en un geste : profil d'alimentation, comportement de suralimentation CPU, préférence GPU, plafonds CPU/RAM des VM, `.wslconfig`, services bruyants, ne-pas-déranger, courbe de ventilation. |
| D8-02 | Profils de référence : `Concentration`, `Build`, `Réunion`, `Présentation`, `Batterie`, `Déplacement`, `Nuit`. |
| D8-03 | **Bascule contextuelle automatique** : secteur/batterie, dock connecté, SSID, application au premier plan, événement d'agenda, plage horaire. Avec bascule manuelle par raccourci global et **annulation toujours possible**. |
| D8-04 | Gestion de la charge de batterie (plafond à 80 % en usage sédentaire, charge complète avant un déplacement détecté à l'agenda), suivi de l'usure et des cycles. |
| D8-05 | Mesure de consommation et **ordonnancement des tâches lourdes selon l'intensité carbone du réseau électrique** (source de données optionnelle et locale). |
| D8-06 | Un changement de profil qui exige un redémarrage de WSL le **dit clairement et ne l'impose jamais** ; il propose de l'appliquer à la prochaine occasion calme. |

### D9 — WSL2 et machines virtuelles *(P1)*

| Réf. | Exigence |
|---|---|
| D9-01 | Distro de référence (*golden*) et **clonage en quelques secondes** depuis ce modèle. |
| D9-02 | Instantané et retour arrière par distribution, avec rétention configurable. |
| D9-03 | Compaction automatique de `ext4.vhdx` sous seuil de fragmentation, pendant une fenêtre calme. |
| D9-04 | `wsl.conf` et `.wslconfig` gérés comme configuration déclarative (D2). |
| D9-05 | Diagnostic réseau WSL en un geste : réseau miroir, DNS, dérive d'horloge, MTU, coexistence VPN. |
| D9-06 | Cartographie des redirections de ports vers l'hôte avec **détection de conflit** avant qu'il ne survienne. |
| D9-07 | Vérification GPU et interface graphique : passage GPU, WSLg, versions de pilotes cohérentes entre hôte et invité. |
| D9-08 | Hygiène Hyper-V : âge des points de contrôle, chaînes de disques différentiels, extinction des VM inactives, bibliothèque de modèles. |
| D9-09 | Windows Sandbox préconfiguré (`.wsb`) pour l'analyse d'un binaire non fiable, et mode invité pour un prêt de machine. |

### D10 — Réseau, docks et périphériques *(P1)*

| Réf. | Exigence |
|---|---|
| D10-01 | Profils réseau : DNS, proxy, VPN, avec vérification du tunnel scindé et détection de fuite DNS. |
| D10-02 | Évaluation de la sécurité du réseau joint : chiffrement, portail captif, DHCP suspect, anomalie ARP, référence de latence et de gigue par SSID. |
| D10-03 | **Restauration de la disposition des écrans par dock** — profil mémorisé et réappliqué à la connexion. *Irritant quotidien à fort impact.* |
| D10-04 | Règles de routage audio par contexte, coupure matérielle caméra et micro avec indicateur d'état. |
| D10-05 | **Liste blanche de périphériques USB** via politique d'installation de périphérique (défense contre les périphériques malveillants), avec mode d'apprentissage puis application. |
| D10-06 | Hygiène Bluetooth : appairages orphelins, périphériques absents depuis N mois. |

### D11 — Docteur d'environnement de développement *(P1)*

| Réf. | Exigence |
|---|---|
| D11-01 | Diagnostic « pourquoi mon build est lent » : analyse en temps réel de Defender sur les répertoires de sources, entrées/sorties, longueur de chemin, mode développeur, liens symboliques, antivirus tiers. |
| D11-02 | Proposition d'exclusions Defender **ciblées et expliquées** avec le risque assumé — **jamais d'exclusion globale**, jamais sans que l'utilisateur voie ce qu'il accepte. |
| D11-03 | Vue transverse de tous les dépôts git : branche, état, non poussé, non validé, sous-modules désynchronisés (alimente D6-04). |
| D11-04 | Vérification de cohérence des chaînes d'outils : versions attendues par projet contre versions installées, `PATH` pollué, doublons d'installation. |
| D11-05 | Inventaire des conteneurs et images : images obsolètes, volumes orphelins, `Dockerfile` pointant sur une base non maintenue. |

### D12 — Coexistence avec la gestion d'entreprise *(P1)*

> **Angle mort d'une conception « poste personnel » appliquée à un poste d'entreprise.** Sans ce domaine, Keystone entre en guerre de politiques avec Intune et perd — en laissant la machine dans un état oscillant.

| Réf. | Exigence |
|---|---|
| D12-01 | Détection de la gestion en place : Intune / MDM, GPO de domaine, Configuration Manager, WSUS, et des politiques qu'ils appliquent. |
| D12-02 | **La MDM est souveraine** (P10). Keystone n'écrase jamais une politique gérée. |
| D12-03 | **Détection de conflit** : un item du yaml qui contredit une politique gérée est signalé comme *conflit*, distinct d'une dérive, et non convergeable. |
| D12-04 | Détection de l'oscillation : un item repoussé par la MDM à chaque cycle est identifié comme tel plutôt que reconverti en boucle. |
| D12-05 | Export d'un rapport de conformité destiné à l'équipe sécurité (formats **OSCAL** et **SARIF**), sans transmission automatique. |
| D12-06 | Mode « poste personnel » et mode « poste géré » avec des jeux de capacités différents et clairement annoncés. |

### D13 — Flotte personnelle multi-machines *(P1)*

| Réf. | Exigence |
|---|---|
| D13-01 | Configuration de base commune + surcouche par machine (D2-10), un seul dépôt git. |
| D13-02 | Vue consolidée de 1 à 5 machines, en lecture, sans serveur central : le dépôt git est le point de rendez-vous. |
| D13-03 | Promotion d'un changement validé d'une machine vers les autres, par demande de fusion. |
| D13-04 | Détection de divergence entre machines censées être identiques. |

### D14 — Timeline forensique et isolement d'urgence *(D14-06 en P0, reste en P2)*

| Réf. | Exigence |
|---|---|
| D14-01 | Timeline unifiée et navigable de tout changement — d'origine utilisateur, Keystone, Windows, MDM ou inconnue. |
| D14-02 | Corrélation avec les incidents : vidages sur incident (`WER`, minidumps) triés automatiquement, décodage du code d'arrêt, événements thermiques, redémarrages. |
| D14-03 | Réponse à la question « qu'est-ce qui a changé avant que ça casse mardi à 14 h » en moins de trois interactions. |
| D14-04 | Horodatage monotone et cohérent inter-systèmes (dépend de D1-09). |
| D14-05 | Journal scellé et exportable comme élément de preuve (empreinte chaînée). |
| D14-06 | **Isolement d'urgence (P0)** : coupure réseau hors canal d'administration, verrouillage de session, capture d'un instantané forensique (mémoire si possible, processus, connexions, artefacts de persistance), scellement du journal. Exige Windows Hello. *Préserve les preuves au lieu de les détruire par un redémarrage réflexe.* |

### D15 — Copilote local et explicabilité *(P2)*

| Réf. | Exigence |
|---|---|
| D15-01 | Chaque item de configuration porte une explication en langage clair : à quoi il sert, quel risque en le changeant, quelle source de référence (P6). |
| D15-02 | Copilote **strictement local** (llama.cpp ou ONNX) en RAG sur le journal, l'inventaire et les journaux d'événements. Aucun appel réseau. |
| D15-03 | Accès outillé en **lecture seule**. Toute action proposée passe par le même portail de simulation et d'approbation qu'une action humaine. **Aucune application automatique, sans exception.** |
| D15-04 | Cas d'usage visés : « pourquoi mon disque est plein », « explique ce code d'arrêt », « qu'est-ce qui a changé depuis hier », « quel est le risque si je désactive ceci ». |
| D15-05 | Le copilote **cite ses sources** dans le journal local et n'invente jamais une valeur d'inventaire : toute affirmation est adossée à une entrée réelle. |

### D16 — Rapports, exports et interopérabilité *(P2)*

| Réf. | Exigence |
|---|---|
| D16-01 | Rapport matinal quotidien : ce qui a changé, ce qui attend, ce qui a été fait pendant la nuit. *(Moment M2 du brief.)* |
| D16-02 | Export SBOM de la machine, rapport de conformité (OSCAL, SARIF), export CSV et JSON de tout inventaire. |
| D16-03 | Crochets sortants (*webhooks*) et exporteur Prometheus, tous deux désactivés par défaut. |
| D16-04 | Inventaire d'actifs : numéro de série, garantie, échéances de licence et d'abonnement. |
| D16-05 | **Consultation mobile en lecture seule** : état du poste depuis un téléphone, via relais sortant explicitement activé, avec une **unique action possible : isoler la machine**. |
| D16-06 | **Budget d'alertes** : au plus 2 interruptions par jour, tout le reste agrégé dans le rapport matinal. Aucun badge de non-lus. *Le respect de l'attention est une exigence fonctionnelle, pas une préférence esthétique.* |

---

## 8. Exigences non fonctionnelles

| Réf. | Domaine | Exigence |
|---|---|---|
| **NF-01** | Empreinte | Au repos : < 1 % de CPU en moyenne sur 5 min, < 150 Mo de mémoire résidente pour le broker, écritures disque < 50 Mo/jour hors instantanés. Un collecteur qui dépasse son budget est mis en veille et signalé. |
| **NF-02** | Réactivité | Ouverture de l'interface < 800 ms jusqu'au premier rendu utile. Réponse de l'API locale < 100 ms au 95ᵉ centile. Un scan complet ne bloque jamais l'interface. |
| **NF-03** | Fiabilité, mode dégradé | Chien de garde surveillant le broker ; **mode sans échec** en lecture seule si le contrôle d'intégrité échoue ; **P7 impératif** : une panne de Keystone ne dégrade en rien l'usage de la machine. Toute opération est reprenable après coupure ou redémarrage. |
| **NF-04** | Vie privée et gouvernance | Aucune donnée sortante par défaut. Rétention configurable par catégorie (inventaire 24 mois, journal 5 ans, télémétrie brute 90 jours). Rédaction des secrets à l'écriture. Purge complète sur demande, en une commande. |
| **NF-05** | Accessibilité | WCAG 2.2 AA. Aucun sens porté par la couleur seule. Navigation clavier intégrale. `prefers-reduced-motion` et `forced-colors` gérés. Équivalent tableau pour tout graphique. Cibles ≥ 24 px. |
| **NF-06** | Internationalisation | Français et anglais dès la v1, aucune chaîne codée en dur, formats de date, d'unité et de nombre localisés. Journal en identifiants stables et localisation à l'affichage. |
| **NF-07** | Testabilité | Tests d'**idempotence** systématiques (D2-09). Tests d'intégration en **VM éphémères** sur une matrice de versions de Windows. Injection de fautes sur chaque chemin de rollback : *un chemin de retour arrière non testé est réputé inexistant*. Couverture des collecteurs ≥ 80 %. |
| **NF-08** | Distribution et amorçage | Binaires signés. **Keystone ne se met pas à jour tout seul.** La mise à jour du broker est atomique, avec vérification de signature, puis test de santé, puis basculement — et retour à la version précédente si le test échoue. Le problème de « qui met à jour l'updater » est résolu par un amorceur minimal, immuable et signé séparément. |
| **NF-09** | Documentation | Chaque exigence fonctionnelle est documentée côté utilisateur avec son *pourquoi* et son *risque*. Les limites du §4.3 figurent dans la documentation d'accueil, pas en annexe. |
| **NF-10** | Observabilité de l'outil | Keystone s'applique à lui-même : ses propres métriques, journaux et santé sont visibles dans son interface. |
| **NF-11** | Compatibilité | Windows 11 24H2+ Pro/Enterprise, x64 et ARM64. Distros WSL2 : Ubuntu, Debian, Fedora, Alpine, Arch. Dégradation propre et explicite si Hyper-V ou TPM est absent. |
| **NF-12** | Licence et gouvernance du projet | Source ouverte sous **Apache-2.0** — tranché, voir [`LICENSE`](../LICENSE) et `Cargo.toml`. Pas de dépendance à un service propriétaire, facteur d'autobus documenté (§14). |

---

## 9. Modèle de données

### 9.1 Squelette de `workstation.yaml`

```yaml
apiVersion: keystone/v1
kind: Workstation
metadata:
  name: WKS-TENE-01
  inherits: ./base.yaml            # surcouche de flotte (D13-01)
  owner: tene

platform:
  secureBoot: enabled
  tpm: { present: true, version: "2.0" }
  vbs: { enabled: true, hvci: enforced }
  lsaProtection: enabled
  bitlocker:
    C: { required: true, method: XtsAes256 }
    D: { required: true, method: XtsAes256 }

updates:
  windows:
    quality: auto                  # correctifs de sécurité automatiques
    features: manual               # jamais de saut de version sans validation
    drivers: pinned                # Windows Update ne touche pas aux pilotes
  securityFastLane: true           # un CVE exploité court-circuite les anneaux (D3-03)
  maintenanceWindow: { days: [Tue, Thu], from: "20:00", to: "23:00" }
  rings:
    canary: [vscode, gh, ripgrep, fd]
    stable: [docker-desktop, nodejs, python]
    manual: [visualstudio, wsl-kernel]
  pinned:
    nvidia-driver:
      version: "566.36"
      reason: "régression CUDA constatée sur la branche 570.x"
      expires: 2026-09-01          # l'épingle expire — pas de gel silencieux (D3-02)
  smokeTests:                      # seul juge du résultat (D3-09)
    - { id: build-ref,  cmd: "ks run build-reference", timeout: 600 }
    - { id: cuda-smoke, cmd: "wsl -d ubuntu-dev -- nvidia-smi", timeout: 30 }
    - { id: vpn-up,     cmd: "ks probe vpn", timeout: 60 }

security:
  defender:
    realtime: enabled
    asrRules: [BlockOfficeChildProcess, BlockCredentialStealing, BlockScriptObfuscation]
    exclusions:                    # ciblées et justifiées uniquement (D11-02)
      - path: "D:\\src\\monorepo\\target"
        reason: "artefacts de build Rust, 400k fichiers, +6 min par build"
        expires: 2027-01-01
  wdac: { mode: audit }
  watch:                           # les diffs qui déclenchent une alerte (D5)
    - rootCertificates              # ajout d'une CA racine
    - persistence                   # Run, services, tâches, WMI, COM
    - listeningPorts
    - egressBaseline
  canaries:                        # détection comportementale (D5-08)
    files: ["D:\\src\\_ne-pas-ouvrir\\credentials.txt"]
    cloudKey: true

backup:                            # D6 — protège le travail, pas la machine
  engine: restic
  targets:
    - { path: "D:\\src",       schedule: hourly, retention: "7d,4w,12m" }
    - { path: "%USERPROFILE%\\Documents", schedule: daily,  retention: "30d,12m" }
  offsite: { enabled: true, encryptedClientSide: true }
  verifyRestore: { schedule: weekly, sampleSize: 50 }   # D6-03
  rpo: 1h
  rto: 4h
  unpushedWorkAlert: { after: 8h }                      # D6-04

wsl:
  distros:
    - name: ubuntu-dev
      from: golden/ubuntu-24.04
      resources: { memory: 16GB, processors: 8, swap: 4GB }
      autoCompact: { whenFragmentationOver: 30 }
      packages: { file: ./wsl/ubuntu-dev.packages }
      snapshots: { schedule: daily, keep: 7 }

profiles:                          # D8 — bascule contextuelle
  Build:      { power: max, boost: aggressive, dnd: true,  services: { stopNoisy: true } }
  Réunion:    { power: balanced, dnd: true, vmCpuCap: 25, notifications: suppressed }
  Batterie:   { power: efficiency, boost: off, chargeLimit: 80 }
  Déplacement: { vpn: forced, usbAllowlistOnly: true, lockTimeout: 60, syncPause: [Sensible] }
  triggers:
    - { when: "onBattery", use: Batterie }
    - { when: "dock == 'TB4-BUREAU'", use: Build, restoreDisplays: true }
    - { when: "calendar.busy", use: Réunion }

acceptedDrift:                     # D2-06 — toujours daté et justifié
  - item: services.Fax.startupType
    reason: "requis temporairement par le driver du scanner du labo"
    expires: 2026-10-15
    decidedBy: tene
    decidedAt: 2026-07-18T09:12:00+02:00
```

### 9.2 Entités principales

| Entité | Rôle |
|---|---|
`Item` | unité de configuration : `domain.path`, valeur voulue, valeur constatée, dernière observation, source, explication, risque |
`Drift` | écart : `Item` + auteur du changement + gravité + statut (active / acceptée / en conflit MDM) |
`Plan` | ensemble ordonné d'`Action` en vagues, avec instantanés, tests de fumée et estimations |
`Action` | verbe typé + paramètres + capacité de simulation + capacité de rollback |
`Snapshot` | point de retour : type, cible, empreinte, taille, expiration |
`JournalEntry` | entrée inaltérable : horodatage monotone, acteur, verbe, diff, résultat, empreinte du précédent |
`Finding` | constat de sécurité : catégorie, source du signal, gravité, technique ATT&CK, statut |
`Decision` | trace de choix humain : acceptation de dérive, épinglage, exclusion — avec raison et expiration |

---

## 10. Interfaces

### 10.1 CLI — `ks` (surface de référence)

```
ks status                      # posture composite et vitaux
ks scan [--domain security]    # collecte, lecture seule
ks diff [--domain updates]     # dérive contre workstation.yaml
ks converge [--apply] [--item ...]
ks plan updates [--ring stable] [--apply]
ks rollback <snapshotId>
ks space [--attribute] [--reclaim --quarantine]
ks quarantine list|restore|purge
ks backup verify|restore <path>
ks wsl clone golden/ubuntu-24.04 nouvelle-distro
ks profile use Réunion
ks journal since 2026-07-28 [--seal]
ks isolate                     # isolement d'urgence, exige Hello
ks explain services.Fax.startupType
ks rebuild --emit-bootstrap
```

La simulation est le comportement par défaut de toute commande mutante ; `--apply` est toujours explicite (P2). **Il n'existe volontairement pas de drapeau `--dry-run`** : un drapeau qu'on peut oublier de passer est un drapeau par lequel on écrit par omission. Seul `--apply` existe, et son absence ne peut donc pas être un accident (voir [`09-GLOSSAIRE.md`](09-GLOSSAIRE.md) § « Simulation ≠ Application »). Toute commande dispose d'une sortie `--json` stable et versionnée.

### 10.2 Interface graphique
Voir [`02-BRIEF-DESIGN.md`](02-BRIEF-DESIGN.md). Contraintes issues du présent document : parité fonctionnelle avec la CLI (P4), commande CLI équivalente affichée dans la palette de commandes, simulation en action primaire, aucun sens porté par la couleur seule, budget de 2 interruptions par jour (D16-06).

### 10.3 API locale
gRPC sur named pipe, contrat versionné, jeton par session, verbes énumérés (SEC-02). Aucun port TCP en écoute (SEC-12).

---

## 11. Roadmap et jalons

| Phase | Durée | Contenu | Sortie de phase |
|---|---|---|---|
| **0 — Observer** | 3 sem. | Broker en **lecture seule**, collecteurs D1, agent WSL, journal, CLI `status`/`scan`, rapport HTML. **Zéro écriture.** Réconciliation d'inventaire D3-01. | La liste des applications non gérées et l'inventaire complet sont produits sur une machine réelle, sans qu'aucun octet n'ait été modifié. |
| **1 — Décrire** | 4 sem. | Schéma du yaml, import de l'existant (D2-02), moteur de diff, git, journal des décisions. **Toujours aucune écriture système.** | Un `workstation.yaml` généré, un diff exact, la dérive suivie sur 7 jours. |
| **2 — Converger** | 6 sem. | Instantanés, rollback, convergence, D3 complet (anneaux, vagues, tests de fumée), D6 sauvegarde. | Une mise à jour volontairement cassée est détectée et annulée automatiquement en moins de 15 min. |
| **3 — Tenir** | 6 sem. | D4 espace, D5 posture et diffs de sécurité, ancres externes (SEC-04/05/06), D14-06 isolement. | Un changement de posture injecté est détecté en moins de 5 min et visible dans le journal externe. |
| **4 — Vivre** | 6 sem. | D8 profils, D9 WSL/VM, D10 réseau et docks, D11 docteur de dev, D12 coexistence MDM. | Une semaine d'usage réel sans intervention manuelle. |
| **5 — Voir** | 8 sem. | Interface Tauri complète (7 écrans du brief), D14 timeline, D16 rapports, D15 copilote local. | Les 8 moments de vérité du brief sont implémentés et testés auprès de 5 utilisateurs. |
| **6 — Essaimer** | 4 sem. | D13 flotte, vue mobile lecture seule, documentation, empaquetage, plan de sortie. | Un tiers installe Keystone à partir de la seule documentation. |

**Séquencement non négociable :** la lecture seule vient d'abord (phases 0 et 1). La confiance se construit avant la première écriture. Un produit qui écrit avant d'avoir prouvé qu'il sait lire ne sera jamais adopté sur une machine de production.

---

## 12. Critères d'acceptation

Ces critères sont vérifiés sur une machine réelle, pas sur maquette.

| # | Critère |
|---|---|
| **A1** | Le premier lancement produit un inventaire complet et un `workstation.yaml` valide **sans qu'aucune donnée système n'ait été modifiée**, vérifié par audit du journal. |
| **A2** | `ks converge --apply` est **idempotent** : la seconde exécution ne rapporte aucun changement. |
| **A3** | Une mise à jour de pilote délibérément défectueuse est installée, détectée par un test de fumée, **annulée automatiquement**, et le composant épinglé — le tout en moins de 15 minutes et sans intervention. |
| **A4** | La désactivation manuelle de la protection en temps réel de Defender est détectée en **moins de 5 minutes**, présente dans le journal externe, et visible en tenue d'attention dans l'interface. |
| **A5** | L'ajout d'une autorité de certification racine est détecté et attribué à sa source, ou marqué **« source inconnue »** si elle est indéterminable. |
| **A6** | La récupération de 40 Go d'espace est **intégralement réversible** pendant 30 jours, restauration vérifiée. |
| **A7** | Un test de restauration de sauvegarde s'exécute automatiquement chaque semaine et **échoue bruyamment** si les empreintes ne correspondent pas. |
| **A8** | La reconstruction complète d'une machine depuis le seul dépôt git aboutit en **moins de 4 heures** non assistées, dans une VM éphémère. |
| **A9** | L'arrêt brutal du broker pendant une convergence laisse la machine dans un état **cohérent et reprenable**, jamais partiel. |
| **A10** | Sur un poste géré par Intune, aucune politique gérée n'est écrasée ; les conflits sont listés comme tels et non convergés. |
| **A11** | L'audit de l'API du broker ne révèle **aucune primitive d'exécution arbitraire** (SEC-02), revue par un tiers. |
| **A12** | Aucun secret n'apparaît dans les journaux, vérifié par un scanner automatisé sur 30 jours de journaux réels. |
| **A13** | L'interface satisfait WCAG 2.2 AA sur les 7 écrans, vérifié par outil **et** par parcours clavier intégral. |
| **A14** | `ks uninstall` retire Keystone et laisse la machine dans l'état documenté, vérifié par diff avant/après. |
| **A15** | Sur 30 jours d'usage réel, l'utilisateur reçoit **au plus 2 interruptions par jour** (D16-06). |

---

## 13. Risques et mitigations

| # | Risque | Gravité | Mitigation |
|---|---|---|---|
| R1 | **Keystone devient le vecteur d'attaque idéal** | Critique | §6 intégral : API à verbes énumérés, séparation de privilèges, signature, journal externe, Hello sur les verbes coûteux, revue tierce (A11). |
| R2 | **Une convergence casse le poste de travail** | Élevée | P3, instantanés obligatoires, tests de fumée, rollback testé par injection de fautes (NF-07), phases 0 et 1 en lecture seule. |
| R3 | **Fatigue d'alerte, l'outil devient du bruit** | Élevée | Budget d'alertes (D16-06), dérive acceptée datée (D2-06), agrégation dans le rapport matinal, aucun badge. |
| R4 | **Le yaml pourrit** : exceptions accumulées, personne ne sait plus pourquoi | Élevée | Expiration obligatoire sur toute exception, journal des décisions (D2-07), revue périodique proposée par l'outil. |
| R5 | **Guerre de politiques avec la MDM** | Élevée | D12 intégral, souveraineté MDM (P10), détection d'oscillation. |
| R6 | **Dérive de périmètre** : le projet ne finit jamais | Élevée | Priorités P0/P1/P2 tenues, sorties de phase avec critère mesurable, refus explicite de tout ce qui figure au §4.2. |
| R7 | **Fausse assurance de sécurité** | Élevée | §4.3 en première page de la documentation, matrice ATT&CK honnête (D5-12), positionnement complémentaire à l'EDR affirmé partout. |
| R8 | **Faux positifs de détection sur un poste de dev** (l'activité normale ressemble à une attaque) | Moyenne | Référence par machine apprise sur 14 jours, seuils par domaine, distinction nette entre « écart » et « constat de sécurité ». |
| R9 | **Rupture d'API Windows** entre versions | Moyenne | Collecteurs isolés — bac à sable WASM pour les tiers, processus et budget contraints pour les nôtres (ADR-0003) — matrice de VM éphémères en intégration continue (NF-07), dégradation propre. |
| R10 | **Facteur d'autobus** : un seul auteur | Moyenne | §14, documentation d'architecture, source ouverte, tout automatisme reproductible à la main. |
| R11 | **Empreinte perçue** : « encore un agent qui rame » | Moyenne | Budgets NF-01 appliqués, collecteur hors budget mis en veille, empreinte affichée dans l'interface (NF-10). |
| R12 | **Le copilote local propose une bêtise et elle est appliquée** | Moyenne | D15-03 : lecture seule, aucune application automatique, même portail d'approbation qu'un humain. |

---

## 14. Plan de sortie

Exigence de dignité du projet : **Keystone ne doit jamais devenir une dépendance dont on ne peut pas se défaire.**

1. `ks uninstall` retire le service, les modules et les politiques posées, et produit un rapport de ce qui a été remis en état (A14, P8).
2. Le `workstation.yaml` et le journal restent **lisibles sans Keystone** : YAML et SQLite, formats documentés.
3. Toute automatisation possède son **équivalent manuel documenté** — la machine reste administrable à la main si le projet s'arrête.
4. Le script de reconstruction (D6-06) ne dépend **pas** de Keystone pour s'exécuter.
5. Architecture, modèle de menace et schémas publiés en source ouverte ; le facteur d'autobus est traité par la documentation, non par la présence de l'auteur.

---

## 15. Glossaire

| Terme | Définition |
|---|---|
| **Convergence** | Action d'amener l'état réel vers l'état désiré déclaré. |
| **Dérive** | Écart mesuré entre l'état désiré et l'état réel. |
| **Dérive acceptée** | Écart volontairement toléré, avec raison et date d'expiration obligatoires. |
| **Conflit** | Écart causé par une politique gérée souveraine (MDM) — non convergeable par conception. |
| **Simulation (*dry-run*)** | Calcul et affichage du diff sans aucune écriture. Comportement par défaut. |
| **Anneau** | Classe de risque déterminant la vitesse d'adoption d'une mise à jour. |
| **Test de fumée** | Vérification définie par l'utilisateur qui juge si un changement est acceptable. |
| **Instantané** | Point de retour arrière technique. **N'est pas une sauvegarde.** |
| **Sauvegarde** | Copie des données de travail selon la règle 3-2-1, avec restauration vérifiée. |
| **Quarantaine** | Zone de rétention temporaire remplaçant la suppression définitive. |
| **Canari** | Leurre dont tout usage révèle un adversaire par son comportement. |
| **Ancre externe** | Référence conservée hors machine, seule capable de contredire un hôte compromis. |
| **Moment de vérité** | Instant d'usage explicitement conçu, où le produit gagne ou perd la confiance de son utilisateur. |
