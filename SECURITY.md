# Politique de sécurité

Keystone administre un poste de travail avec les droits les plus élevés de la machine.
**Compromis, il constitue l'outil d'attaque le plus efficace imaginable sur ce poste.**

Ce n'est pas une formule. C'est la raison d'être de ce document, et la raison pour
laquelle le modèle de menace complet — avec ses angles morts assumés — est publié :
[`docs/04-MODELE-DE-MENACE.md`](docs/04-MODELE-DE-MENACE.md).

---

## Signaler une vulnérabilité

**Ne pas ouvrir d'issue publique.** Une issue est indexée avant d'être corrigée.

| | |
|---|---|
| **Où** | [Avis de sécurité privé](https://github.com/ten-tech/keystone/security/advisories/new) — le canal confidentiel de GitHub |
| **Accusé de réception** | sous 72 heures |
| **Première évaluation** | sous 7 jours |
| **Divulgation** | coordonnée, après correctif — ou à 90 jours, selon ce qui arrive en premier |

> Le signalement privé doit être activé dans les réglages du dépôt
> (*Settings → Code security → Private vulnerability reporting*) pour que ce lien
> fonctionne. Tant qu'il ne l'est pas, il n'existe aucun canal confidentiel, et
> l'invitation à ne pas ouvrir d'issue publique est une invitation sans issue.

### Ce qui aide un rapport

Ce qui s'est passé, ce que ça implique, ce qu'il faudrait faire — dans cet ordre,
comme tout message du projet ([`docs/08-CONVENTIONS.md`](docs/08-CONVENTIONS.md)) :

- la version ou le commit, et la plateforme ;
- les étapes de reproduction, aussi courtes que possible ;
- l'impact réel — *ce qu'un attaquant obtient*, pas la classe de bug ;
- l'adversaire concerné, s'il figure au §3 du modèle de menace (A1 à A6).

**Aucun secret dans un rapport.** Si la reproduction en exige un, dites-le plutôt que
de le joindre. Le projet applique la même règle à ses propres journaux : la rédaction
se fait à l'écriture, jamais à l'affichage.

---

## Périmètre

### Dans le périmètre, par ordre de gravité

1. **Toute exécution de code arbitraire par l'API du broker.** C'est l'angle mort que
   toute l'architecture existe pour fermer (SEC-02). Un verbe qui, directement ou par
   détour, permet d'exécuter du code est une faille critique — y compris s'il est
   « seulement » accessible à l'utilisateur interactif.
2. **Franchissement de la frontière de privilège** : un client non élevé qui obtient
   du broker plus que ses verbes (SEC-01, ACL du named pipe).
3. **Falsification ou effacement du journal**, local ou expédié (SEC-03, SEC-04).
4. **Contournement de la simulation obligatoire** (P2) ou du filet de sécurité (P3) :
   toute écriture système qui n'a pas été montrée d'abord, ou qui n'est pas annulable.
5. **Fuite de secret** : clé de sauvegarde, jeton, clé de récupération BitLocker —
   dans un journal, un rapport, un export, une trace.
6. **Contournement de la vérification d'intégrité au démarrage** (SEC-07), ou
   chargement d'un module non signé.
7. **Toute surface réseau en écoute** (SEC-12). Le broker n'écoute jamais.
8. **Faux négatif de posture** : Keystone déclare saine une machine dont une
   protection a été désactivée. Le produit vaut par ce constat ; le manquer le vide
   de sa raison d'être.

### Hors périmètre — structurellement, et c'est documenté

Ces limites sont **annoncées en première page de la documentation utilisateur**
(exigence NF-09), pas reléguées en annexe. Un outil de sécurité qui laisse croire
qu'il voit tout est plus dangereux qu'un outil qui annonce ses limites.

- Rootkit noyau ou firmware — vit *sous* l'agent.
- Attaque purement en mémoire — c'est le domaine de l'EDR.
- Vol de jeton OAuth ou de cookie par canal légitime.
- Hyperviseur ou hôte compromis — Keystone est l'invité.
- Attaque physique : DMA, cold boot, evil maid.

**Keystone n'est pas un EDR et ne le remplace pas.** L'EDR traque le comportement
malveillant ; Keystone garantit que la posture de l'EDR n'a pas été sabotée, et que
rien n'a changé sans que l'utilisateur le sache. C'est un créneau étroit et réel ; le
prétendre plus large serait mentir.

Le détail complet, adversaire par adversaire, est au §5 du modèle de menace.

---

## Pour les contributeurs

La lecture de [`docs/04-MODELE-DE-MENACE.md`](docs/04-MODELE-DE-MENACE.md) est
**obligatoire** avant toute contribution à `ks-broker`.

### Les verbes interdits, définitivement

```
RunCommand { cmd }         → c'est une porte dérobée
RunScript { path }         → la même, avec un détour
Eval { expression }        → non
SetRegistryValue sans ACL  → équivaut à RunCommand via IFEO
DisableDefender            → n'existe pas comme verbe atomique
```

Si un besoin semble exiger l'un de ces verbes, **le besoin est mal formulé**. Ouvrir
une ADR plutôt qu'un raccourci.

### Les quatre questions pour tout nouveau verbe

À documenter dans son ADR, sans exception, après la question éliminatoire *« ce verbe
permet-il, directement ou par détour, d'exécuter du code arbitraire ? »* :

1. Sait-il **se simuler** ? Sinon il ne peut pas être appliqué.
2. Sait-il **s'annuler** ? Sinon il ne peut pas être automatisé.
3. Exige-t-il une **présence humaine** ? Si son coût est réel, oui.
4. Est-il **idempotent** ? Sinon, pourquoi, et comment le rendre sûr malgré tout.

### Dépendances

Toute nouvelle dépendance de `ks-broker` passe par une ADR, et la question
« peut-on faire sans ? » est posée sérieusement à chaque fois.

```powershell
cargo audit        # vulnérabilités connues
cargo deny check   # licences, doublons, sources
```

Les deux tournent en intégration continue.
