# ADR-0002 — gRPC sur named pipe, et aucun port réseau

- **Statut** : Accepté
- **Date** : 2026-07-30
- **Exigences concernées** : SEC-01, SEC-02, SEC-12, D16-05

## Contexte

Les clients — la CLI, l'interface Tauri, plus tard une vue mobile en lecture seule —
sont **non privilégiés**. Ils doivent parler au broker, qui est élevé. Ce canal est la
frontière de privilège du produit : c'est le point d'entrée qu'un attaquant local
cherchera en premier.

Trois exigences le contraignent :

- le contrat doit être **typé** — un canal qui transporte du JSON libre invite à
  élargir l'API par accident (SEC-02) ;
- l'accès doit être **contrôlable par le système d'exploitation** ;
- il ne doit exister **aucune surface réseau** (SEC-12).

## Décision

**gRPC sur named pipe Windows**, avec un jeton par session.

- Le pipe porte une ACL restreignant l'accès à l'utilisateur interactif de la session.
- Aucun port TCP n'est ouvert, jamais, y compris en écoute locale.
- Le contrat protobuf est versionné : un client trop ancien est refusé explicitement,
  pas silencieusement toléré.
- Pour les VM, le transport équivalent est **Hyper-V Socket** (`AF_HYPERV`) ; pour les
  distros WSL2, un socket Unix relayé par l'hôte.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| **HTTP local sur `127.0.0.1`** | La solution la plus simple, et la pire ici : tout processus de la machine peut s'y connecter, y compris un script dans un onglet de navigateur via une requête forgée. Il faudrait réinventer l'authentification que les ACL du pipe donnent gratuitement. Et ça viole SEC-12 littéralement. |
| **COM / DCOM** | C'est le mécanisme historique de Windows pour exactement ce besoin, avec un modèle de sécurité mûr. Mais la surface d'attaque de DCOM est vaste et régulièrement visitée par les chercheurs, l'activation est configurable de façon inattendue, et les détournements COM sont précisément une des choses que Keystone surveille (D5-02). Utiliser COM pour surveiller les abus de COM est un choix inconfortable. |
| **JSON-RPC sur named pipe** | Garde le bon transport mais perd le contrat typé. Un champ ajouté à la main ne casse rien, ce qui est exactement le mécanisme par lequel une API s'élargit jusqu'à devenir dangereuse. |
| **Fichier de commandes surveillé** | Simple, robuste, sans dépendance — mais aucune authentification de l'appelant, et une course entre écriture et lecture qui se prête aux attaques par lien symbolique. |
| **Service WCF / .NET Remoting** | Impose un runtime et va contre ADR-0001. |
| **Socket TCP pour la vue mobile** | Tentant, et refusé : la consultation à distance (D16-05) passera par un **relais sortant explicitement activé**, initié par la machine. Le broker n'écoute jamais. |

## Conséquences

### Ce que ça nous donne

- Aucune surface réseau à défendre, ni à auditer, ni à expliquer à l'équipe sécurité.
- Le contrôle d'accès est délégué au système, avec des ACL éprouvées.
- Le contrat protobuf rend l'élargissement de l'API **visible dans le diff** : ajouter
  un verbe modifie un fichier `.proto` que la revue remarque forcément.
- Génération de client dans n'importe quel langage — utile pour la future interface.
- Hyper-V Socket permet d'atteindre une VM **au moment où son réseau est cassé**,
  c'est-à-dire quand on en a le plus besoin.

### Ce que ça nous coûte

- Le support gRPC sur named pipe demande un transport personnalisé côté Rust : `tonic`
  attend un flux TCP par défaut, il faut lui fournir un `AsyncRead + AsyncWrite` sur le
  pipe. C'est une soirée de travail, une fois.
- `grpcurl` et les outils de diagnostic habituels ne fonctionnent pas hors de la boîte.
- La vue mobile devra construire un relais, ce qui est plus complexe qu'exposer un port.
  C'est le but.

### Ce que ça ferme

Tout accès distant direct au broker. Définitivement, et par conception.
