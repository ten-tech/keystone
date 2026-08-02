# 09 — Glossaire

Les mots de ce projet ont un sens précis. Les confondre produit des bugs de
conception, pas seulement des malentendus.

## Les distinctions qui comptent

### Instantané ≠ Sauvegarde

| | **Instantané** (`Snapshot`) | **Sauvegarde** (`BackupSet`) |
|---|---|---|
| Protège | la **machine** | le **travail** |
| Sert à | revenir à l'état d'avant une opération | retrouver des données perdues |
| Durée de vie | heures ou jours | mois ou années |
| Exemple | point de contrôle Hyper-V, `wsl --export` | Restic vers un dépôt hors site |

Les confondre était l'angle mort de la première conception du produit : des instantanés
de rollback ne protègent **rien** du travail. D'où le domaine D6, et deux types
distincts dans le code. L'interface ne les mélange jamais dans la même liste.

### Dérive ≠ Conflit

| | **Dérive** (`DriftStatus::Active`) | **Conflit** (`DriftStatus::Conflict`) |
|---|---|---|
| Cause | un changement, quelqu'un ou quelque chose | une politique gérée souveraine (Intune, GPO) |
| Convergeable | oui | **non, jamais** (principe P10) |
| Action | faire converger, ou accepter | signaler, et documenter |

Traiter un conflit comme une dérive déclenche une guerre de politiques avec la MDM que
Keystone perd toujours — en laissant la machine osciller à chaque cycle.

### Simulation ≠ Application

`--dry-run` est le **comportement par défaut** ; il n'existe pas comme drapeau.
C'est `--apply` qui existe. On ne peut donc pas écrire par omission (principe P2).

### Récupérer ≠ Supprimer

Keystone ne supprime pas. Il déplace en **quarantaine**, avec une durée de vie de
30 jours et une restauration en un geste. La purge définitive exige Windows Hello,
parce que son coût est réel.

---

## Vocabulaire du produit

| Terme | Définition |
|---|---|
| **Ancre externe** | référence conservée hors machine — journal expédié, battement de cœur, PCR de référence. Seule chose capable de contredire un hôte compromis. |
| **Anneau** | classe de risque déterminant la vitesse d'adoption d'une mise à jour : `canary`, `stable`, `manual`. |
| **Attribution** | répondre à « *qui* consomme mon disque » ou « *qui* a fait ce changement ». Le contraire d'un « nettoyage » ou d'une alerte anonyme. |
| **Battement de cœur** | signal périodique attendu par le puits externe. **Son absence est l'alerte**, pas un silence neutre. |
| **Canari** | leurre — fichier, identifiant, compte — dont tout usage révèle un adversaire par son comportement, y compris un adversaire inconnu. |
| **Convergence** | amener l'état réel vers l'état désiré déclaré. |
| **Couloir sécurité** | mécanisme par lequel une vulnérabilité critique et activement exploitée court-circuite les anneaux. |
| **Dérive acceptée** | écart volontairement toléré, avec **raison et date d'expiration obligatoires**. Sans expiration, un fichier d'état pourrit en trois ans. |
| **État désiré** | ce que `workstation.yaml` déclare. Unique source de vérité. |
| **Item** | unité de configuration observée. Porte toujours sa provenance, sa finalité et son risque. |
| **Moment de vérité** | instant d'usage explicitement conçu, où le produit gagne ou perd la confiance de son utilisateur. Les huit sont au §5 du brief. |
| **Oscillation** | item repoussé par la MDM à chaque cycle. Identifié comme tel plutôt que reconverti en boucle. |
| **Plan** | ensemble ordonné de vagues, montré intégralement avant d'être appliqué. |
| **Provenance** | qui a produit la valeur constatée. `Observed` dit « relevé, sans prétention sur l'auteur » ; `Unknown` dit « un changement a eu lieu et personne n'en est l'auteur » — c'est **le** signal du produit, et confondre les deux le rend muet. |
| **Quarantaine** | zone de rétention temporaire remplaçant la suppression. |
| **Réconciliation d'inventaire** | croiser toutes les sources d'installation pour produire la liste des applications gérées par **aucun** gestionnaire. Le livrable le plus sous-estimé. |
| **Test de fumée** | vérification définie par l'utilisateur qui juge si un changement est acceptable. Seul juge du résultat. |
| **Vague** | groupe d'actions appliquées ensemble puis jugées. Deux composants couplés ne sont jamais dans la même vague. |
| **Verbe** | opération typée et énumérée exposée par le broker. Il n'existe aucun verbe d'exécution libre. |

---

## Vocabulaire de l'interface

| Terme | Définition |
|---|---|
| **Health Ring** | l'anneau central. Une seule valeur composite, toujours dépliable en ses composantes. Respire à 0,25 Hz, et **ralentit** quand l'état se dégrade. |
| **Budget d'alertes** | au plus 2 interruptions par jour. Tout le reste s'agrège dans le rapport matinal. |
| **Tenue d'attention** | changement d'état visuel de toute l'interface quand un signal apparaît — un filet de 2 px, pas une bannière. Rien ne clignote jamais. |
| **Le calme est la fonctionnalité** | la thèse UX. La récompense offerte à l'utilisateur est un écran silencieux, pas une stimulation. |

---

## Faux amis

| Ce qu'on pourrait dire | Pourquoi c'est faux ici |
|---|---|
| « nettoyer le disque » | on **attribue** puis on **récupère vers la quarantaine**. « Nettoyer » est le vocabulaire des outils qu'on ne veut pas imiter. |
| « optimiser » | ne veut rien dire. Chaque action doit avoir un effet mesurable et une source citable (principe P9). |
| « score de santé » | interdit s'il n'est pas dépliable en ses composantes exactes (principe P6). Un chiffre inexplicable est une décoration. |
| « corriger un problème » | on parle d'**écart**, pas de problème. Le mot « problème » culpabilise ; « écart » informe. |
| « menace détectée » | on décrit **un fait, une heure, une conséquence, une action**. Le vocabulaire de la peur est un dark pattern. |
| « Keystone détecte les intrusions » | faux, et dangereux. Keystone garantit que la posture de l'EDR n'a pas été sabotée et que rien n'a changé sans qu'on le sache. Voir §5 du modèle de menace. |
