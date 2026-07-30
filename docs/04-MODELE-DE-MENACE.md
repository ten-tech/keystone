# 04 — Modèle de menace

> **Lecture obligatoire avant toute contribution à `ks-broker`.**
>
> Révision exigée à chaque version majeure (exigence SEC-13).

---

## 1. L'énoncé du problème

Keystone administre tout, avec les droits les plus élevés de la machine : services,
registre, politiques, Defender, BitLocker, TPM, WSL, Hyper-V, réseau, périphériques.

> **Compromis, Keystone constitue l'outil d'attaque le plus efficace imaginable sur
> ce poste.**

Ce n'est pas une formule d'introduction. C'est la contrainte qui détermine
l'architecture : si le broker était un service généraliste capable d'exécuter des
commandes, il n'y aurait plus aucune différence entre Keystone et une porte dérobée
signée, démarrée automatiquement, et à laquelle l'utilisateur fait confiance.

## 2. Ce qu'on protège

| Actif | Pourquoi il compte |
|---|---|
| **L'API du broker** | l'exécution privilégiée. La compromettre, c'est tout obtenir. |
| **Le journal** | c'est la seule chose qui permet de dire *ce qui s'est passé*. |
| **`workstation.yaml`** | modifier l'état désiré, c'est faire converger la machine vers une configuration hostile — en toute légitimité apparente. |
| **La clé de signature des modules** | signer un module malveillant, c'est passer tous les contrôles. |
| **Les instantanés et les sauvegardes** | les détruire, c'est supprimer la possibilité de revenir. |
| **Les secrets scellés** | clés de sauvegarde hors site, jetons. |

## 3. Les adversaires

| # | Adversaire | Capacités supposées | Traité par |
|---|---|---|---|
| **A1** | Code utilisateur malveillant sans élévation | exécution dans le contexte de l'utilisateur, lecture de son profil | SEC-01, SEC-02, ACL du named pipe |
| **A2** | Attaquant ayant obtenu SYSTEM | tout sur la machine, y compris réécrire le journal local et tuer le service | SEC-04, SEC-05, SEC-06 — **les ancres externes** |
| **A3** | Contributeur malveillant ou négligent | ajoute un verbe trop puissant, une dépendance compromise | revue par ADR, test SEC-02, épinglage des dépendances |
| **A4** | Chaîne d'approvisionnement | crate compromise, module tiers | `cargo-audit`, `cargo-deny`, WASM en bac à sable, signature |
| **A5** | Adversaire physique | DMA, cold boot, evil maid | **hors périmètre** — voir §5 |
| **A6** | Keystone lui-même, mal configuré | l'utilisateur écrit un yaml qui affaiblit sa propre machine | explicabilité P6, risque affiché par item, Hello sur les verbes coûteux |

Le cas **A2** est le plus intéressant, parce qu'il est *déjà perdu* localement. On ne
prétend pas empêcher un attaquant SYSTEM d'agir : on garantit qu'il ne peut pas
effacer la trace de son passage, parce qu'elle est déjà ailleurs.

## 4. Les contre-mesures, et ce qu'elles couvrent

| Réf. | Contre-mesure | Adversaire visé |
|---|---|---|
| SEC-01 | Séparation de privilèges : UI et CLI non élevées | A1 |
| SEC-02 | **API de verbes typés et énumérés.** Aucune primitive d'exécution libre | A1, A3 |
| SEC-03 | Journalisation exhaustive : appelant, verbe, paramètres, diff, résultat | tous |
| SEC-04 | **Journal expédié hors machine, destination en écriture seule** | A2 |
| SEC-05 | **Battement de cœur — l'absence est l'alerte** | A2 |
| SEC-06 | **Attestation TPM contre référence externe** (PCR, DRTM) | A2, A5 partiellement |
| SEC-07 | Binaires et modules signés ; intégrité vérifiée au démarrage ; échec → lecture seule | A3, A4 |
| SEC-08 | Windows Hello sur les verbes à coût réel | A1, A6 |
| SEC-09 | Secrets scellés au TPM ; **rédaction à l'écriture**, pas à l'affichage | A1, A2 |
| SEC-10 | Limitation de débit sur les verbes destructeurs | A1, A6 |
| SEC-11 | Pas d'auto-mise-à-jour silencieuse | A4 |
| SEC-12 | **Aucun port réseau en écoute** | A1 |

## 5. Ce qui n'est pas couvert — et le sera jamais

Cette liste doit figurer **en première page de la documentation utilisateur**, pas en
annexe (exigence NF-09). Un outil de sécurité qui laisse croire qu'il voit tout est
plus dangereux qu'un outil qui annonce ses limites.

| Angle mort | Pourquoi c'est structurel | Ce qu'on peut quand même faire |
|---|---|---|
| **Rootkit noyau ou firmware** (UEFI, option ROM, BMC, firmware SSD/NIC) | vit *sous* l'agent | constater une divergence de PCR (SEC-06) — sans voir le malware |
| **Attaque purement en mémoire** | ne laisse aucun artefact persistant | rien. C'est le domaine de l'EDR. |
| **Vol de jeton OAuth ou de cookie de session** | canal légitime, aucun artefact système | suivi d'expiration (D7-02), canaris (D5-08) |
| **Hyperviseur ou hôte compromis** | Keystone est l'invité | rien |
| **Attaque physique** (DMA, cold boot, evil maid) | machine hors du contrôle de l'agent | BitLocker + protection DMA + attestation au démarrage |
| **Zero-day dans Keystone** | on est le logiciel | surface minimale, revue, journal externe |

### Le positionnement, à répéter partout

**Keystone n'est pas un EDR et ne le remplace pas.**

- L'EDR traque le **comportement** malveillant, avec un moteur signé par le noyau.
- Keystone garantit que **la posture de l'EDR n'a pas été sabotée**, et que rien n'a
  changé sur la machine sans que l'utilisateur le sache.

Beaucoup d'intrusions réelles commencent par « quelqu'un a désactivé la protection en
temps réel ». **Ça**, Keystone le voit en moins de cinq minutes (critère A4). C'est un
créneau étroit et réel ; le prétendre plus large serait mentir.

## 6. Ce qui détecte le mieux, en pratique

Par ordre de rapport valeur / effort, d'après ce que les intrusions réelles laissent
comme traces :

1. **Ajout d'une autorité de certification racine** — signature quasi certaine d'une
   interception TLS. Peu de faux positifs, valeur énorme.
2. **Désactivation d'une protection** (temps réel, ASR, LSA, HVCI, BitLocker) — début
   de la plupart des chaînes d'attaque.
3. **Nouvelle persistance** sans auteur identifié — Run, service, tâche, abonnement
   WMI, détournement COM.
4. **Canaris déclenchés** — détecte un adversaire *inconnu* par son comportement, pas
   par une signature.
5. **Silence de l'agent** — le signal que personne n'implémente et qui vaut cher.
6. **Divergence de PCR** — la seule chose qui puisse contredire un hôte compromis.

Et à l'inverse, ce qui **détecte mal sur un poste d'ingénieur** : le
*living-off-the-land* (`certutil`, `rundll32`, PowerShell encodé). C'est visible via
ETW, mais un développeur fait des choses bizarres toute la journée : le taux de faux
positifs rend le signal inexploitable. On le journalise pour la forensique, on n'en
fait pas une alerte.

## 7. Règles pour les contributeurs

### Les verbes interdits, définitivement

```
RunCommand { cmd }              → c'est une porte dérobée
RunScript { path }              → la même, avec un détour
Eval { expression }             → non
SetRegistryValue sans ACL       → équivaut à RunCommand via IFEO
DisableDefender                 → n'existe pas comme verbe atomique :
                                  passe par la convergence, avec diff,
                                  instantané et Windows Hello
```

Si un besoin semble exiger l'un de ces verbes, **le besoin est mal formulé**. Ouvrir
une ADR plutôt qu'un raccourci.

### Les quatre questions pour tout nouveau verbe

À documenter dans son ADR, sans exception :

1. Sait-il **se simuler** ? Sinon il ne peut pas être appliqué.
2. Sait-il **s'annuler** ? Sinon il ne peut pas être automatisé.
3. Exige-t-il une **présence humaine** ? Si son coût est réel, oui.
4. Est-il **idempotent** ? Sinon, pourquoi, et comment le rendre sûr malgré tout.

### Les journaux ne contiennent jamais de secret

La rédaction se fait **à l'écriture**, jamais à l'affichage. Un secret masqué à
l'écran mais présent sur disque est un secret sur disque. Le critère A12 le vérifie
sur 30 jours de journaux réels.

### Les dépendances

```powershell
cargo install cargo-audit cargo-deny
cargo audit     # vulnérabilités connues
cargo deny check  # licences, doublons, sources
```

Les deux tournent en intégration continue. Pour le broker, toute nouvelle dépendance
doit être justifiée : sur un composant privilégié, chaque crate ajoutée est une
surface ajoutée.

## 8. Journal des révisions

| Date | Version | Changement |
|---|---|---|
| 2026-07-30 | 1.0 | Rédaction initiale, en même temps que le squelette de Phase 0. |
