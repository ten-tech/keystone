# ADR-0005 — Lire l'état effectif des protections par WMI, et rien de plus

- **Statut** : Proposé
- **Date** : 2026-08-02
- **Exigences concernées** : D5, D1-10, P6, SEC-01

## Contexte

Le collecteur de posture lit le registre, et le registre ne dit qu'une chose :
la **configuration**. Aucune valeur n'y atteste qu'une protection *tourne*.

La distinction n'est pas théorique. Une machine où l'intégrité mémoire est
configurée mais dont le noyau sécurisé n'a pas démarré — pilote incompatible,
refus côté hyperviseur, matériel non éligible — présente exactement la même
configuration qu'une machine protégée. Keystone affiche « activé » dans les deux
cas. C'est la garantie annoncée non tenue que ce projet passe son temps à
corriger, et elle porte ici sur l'item que le §6 du modèle de menace classe au
deuxième rang de valeur.

Microsoft documente une classe WMI pour cela, `Win32_DeviceGuard`, dont la page
« Validate enabled VBS and memory integrity features » donne les tables de codes
exactes. `VirtualizationBasedSecurityStatus` y distingue **1, activé mais pas en
cours d'exécution**, de **2, activé et en cours d'exécution**. Le registre ne
porte pas cette nuance ; c'est toute la raison de cette ADR.

### Ce qui a été mesuré, et ce qui contredit la documentation

En session **non élevée**, c'est-à-dire dans le contexte où la CLI s'exécute
réellement (SEC-01) :

| Classe | Espace de noms | Résultat mesuré |
|---|---|---|
| `Win32_DeviceGuard` | `root\Microsoft\Windows\DeviceGuard` | **lisible** |
| `MSFT_MpComputerStatus` | `root\Microsoft\Windows\Defender` | **lisible** |
| `Win32_Tpm` | `root\CIMV2\Security\MicrosoftTpm` | **accès refusé** |
| `Win32_EncryptableVolume` | `root\CIMV2\Security\MicrosoftVolumeEncryption` | **accès refusé** |

Deux enseignements, et le second est le plus important.

La documentation Microsoft dit que `Win32_DeviceGuard` « peut être interrogée
depuis une session PowerShell **élevée** ». La mesure dit le contraire : un
utilisateur ordinaire y accède. Tant mieux, mais cela signifie qu'on dépend d'un
comportement plus permissif que celui qui est documenté, et qu'un durcissement
futur de Windows pourrait le retirer. Le code doit donc traiter le refus comme
un cas nominal, pas comme une anomalie.

Et surtout : **le TPM et BitLocker par volume ne sont pas débloqués par cette
décision.** Ils ne butent pas sur l'absence d'API, mais sur une liste de contrôle
d'accès. Aucun choix de bibliothèque ne les rendra lisibles sans élévation. Ils
appartiennent au broker, donc à la Phase 2. La feuille de route les y déplace.

## Décision

`ks-collectors` lit **deux classes WMI, en lecture seule**, pour publier l'état
effectif de VBS, de l'intégrité mémoire, de Credential Guard et de la protection
en temps réel de Defender.

Aucune méthode WMI n'est invoquée : uniquement des requêtes `SELECT`. Un
collecteur observe, il ne déclenche rien — invoquer une méthode WMI serait un
effet de bord, donc une infraction à la règle du crate.

Les items portent des chemins qui **disent lequel des deux niveaux ils
décrivent** : `security.platform.vbs_policy` reste la configuration lue au
registre, `security.platform.vbs_running` devient l'état effectif. Les deux
coexistent, et leur divergence est précisément l'information intéressante.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| **Ne rien faire, garder le registre seul** | Laisse le produit afficher « activé » sur une machine non protégée. C'est le défaut le plus grave qu'un outil de posture puisse avoir, et il porte sur son item le mieux noté. |
| **Appeler `powershell.exe` ou `wmic.exe`** | Exclu par SEC-02 : aucune primitive d'exécution libre. Et un collecteur ne lance pas de processus (NF-01) — le coût se compte en secondes. |
| **Lire `Win32_Tpm` et `Win32_EncryptableVolume` par la même voie** | Mesurés en accès refusé sans élévation. Les inclure ferait croire que la décision les couvre, alors qu'ils attendent le broker. |
| **COM brut via le crate `windows`** | Fonctionne, mais ramène `unsafe` dans `ks-collectors`, qui n'en contient aucun bloc à ce jour. L'acquis vaut d'être défendu tant qu'une API sûre existe. |
| **Déduire l'exécution depuis le registre** | Il n'existe aucune valeur documentée qui l'atteste. Toute déduction serait une invention, et une invention rassurante. |

## Conséquences

### Ce que ça nous donne

Quatre items qui disent enfin ce qui **tourne**, et non ce qui est écrit :

* `security.platform.vbs_running` — 0 éteint, 1 configuré mais **pas en cours
  d'exécution**, 2 en cours d'exécution ;
* `security.platform.hvci_running` — l'intégrité mémoire, code 2 dans
  `SecurityServicesRunning` ;
* `security.platform.credential_guard_running` — code 1 dans la même liste, ce
  qui lève enfin l'ambiguïté de l'absence au registre, où « éteint » et « actif
  par défaut depuis 22H2 » se ressemblaient ;
* `security.defender.realtime` — le chemin que `ks-core` donne depuis toujours
  comme **exemple canonique** d'item, et que le collecteur ne produisait pas.

En prime, sans coût supplémentaire : la disponibilité de la protection DMA
(`AvailableSecurityProperties`, code 3) et le mode d'application de la stratégie
d'intégrité du code.

### Ce que ça nous coûte

Une dépendance de plus, et l'initialisation de COM dans le processus de la CLI —
un effet observable sur le processus hôte, à cantonner et à documenter.

Un régime d'erreur supplémentaire : WMI peut échouer pour des raisons qui n'ont
rien à voir avec la sécurité, dépôt corrompu ou service arrêté. Ces échecs se
rangent en [`ItemValue::Illisible`], comme les refus du registre. La règle du
module ne change pas : **on n'invente jamais une valeur qu'on n'a pas lue.**

### Ce que ça ferme

Rien. Le jour où le broker existera, il lira le TPM et BitLocker par la même
bibliothèque, avec l'élévation en plus. Cette ADR sera alors complétée, pas
remplacée.

### Ce qui reste explicitement non couvert

* **TPM** — accès refusé sans élévation. Phase 2.
* **BitLocker par volume**, protecteurs et présence de la clé de récupération —
  même raison, même phase.
* **Protection DMA effective** — seule sa *disponibilité* matérielle est lisible,
  pas son activation.
* **Usure NVMe et SMART** — `DeviceIoControl`, donc `unsafe`, donc une autre
  décision.

Ces quatre points doivent figurer dans la documentation utilisateur au titre du
§5 du modèle de menace. Un utilisateur qui croit son TPM vérifié parce que
Keystone parle de sécurité prendrait une décision sur une base fausse.
