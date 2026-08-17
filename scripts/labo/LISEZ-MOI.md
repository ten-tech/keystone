# Le labo, quand l'hôte n'a pas Hyper-V

Ces deux fichiers installent un Windows 11 Pro jetable **sans intervention**,
dans une distribution WSL2 dédiée, sur un poste qui n'a pas Hyper-V.

Ils existent parce que le poste de référence tourne sous Windows 11 **Famille** :
`New-VM`, `Enable-VMTPM` et `PowerShell Direct` n'y existent pas, donc la
procédure de [`docs/06-VM-DE-LABO.md`](../../docs/06-VM-DE-LABO.md) y est
inapplicable. Voir ce document pour ce qui a été mesuré, et pour ce qui reste à
prouver.

## Préparer, une fois

```powershell
wsl --install -d Debian --name ks-lab --location C:\wsl\ks-lab --no-launch
```

Une distribution **séparée** de celle de travail. Ce n'est pas de la coquetterie :
une distribution dans laquelle on installe un hyperviseur cesse d'être une
distribution de travail, et le labo n'a de valeur que si on peut le détruire
sans réfléchir.

```bash
apt-get install -y --no-install-recommends \
  qemu-system-x86 qemu-utils swtpm swtpm-tools ovmf xorriso socat netpbm
```

Puis, dans la distribution : placer une image de Windows 11 en
`/srv/lab/vm/win11.iso`, construire le support de réponses, et lancer.

```bash
mkdir -p /srv/lab/{tpm,vm,rep}
cp autounattend.xml /srv/lab/rep/
xorriso -as mkisofs -J -r -V UNATTEND -o /srv/lab/vm/unattend.iso /srv/lab/rep
swtpm_setup --tpm2 --tpmstate /srv/lab/tpm --create-ek-cert --create-platform-cert --overwrite
bash demarrer-le-labo.sh
```

**Copiez l'image dans `/srv/lab/vm/`, ne la lisez pas depuis `/mnt/c`.** Le pont
9p vers le disque Windows est d'un ordre de grandeur plus lent, et c'est ce qui
faisait paraître le chargement bloqué.

## Faire démarrer sur le DVD

L'amorceur UEFI de Windows exige une frappe, et sans elle il retombe en PXE au
bout de cinq secondes. Il n'y a pas d'API pour ça : on envoie des frappes par le
moniteur QEMU jusqu'à ce que **le disque grossisse**.

```bash
for i in $(seq 1 300); do
  printf 'sendkey ret\n' | socat - UNIX-CONNECT:/srv/lab/vm/mon.sock > /dev/null
  sleep 0.5
done
```

Le critère d'arrêt est le disque, pas une durée : une capture d'écran de
firmware ne distingue pas « en train de démarrer » de « bloqué depuis quatre
minutes ». Un octet écrit, si.

## Ce que l'installation fait

Efface le disque, crée la disposition UEFI/GPT, installe **Windows 11 Pro**
(sélectionné par nom, pas par index), et ouvre une session locale `lab` sans
compte Microsoft. Compter une trentaine de minutes.

Pro et pas Famille, et ce n'est pas un confort : **Hyper-V n'existe pas en
Famille**, et c'est exactement ce qu'il reste à éprouver dans l'invité.

## Les deux points de retour, et pourquoi le second existe

```bash
qemu-img snapshot -l /srv/lab/vm/ks-lab.qcow2   # les lister
qemu-img snapshot -a prete /srv/lab/vm/ks-lab.qcow2   # y revenir, VM éteinte
```

| Marque | Ce qu'elle contient |
|---|---|
| `clean` | disque juste après l'installation, **mises à jour du 4 août non finalisées** |
| `prete` | après finalisation et première ouverture de session ; c'est celui qu'on veut |

Revenir à `clean` ramène la machine à un état qui redemande une trentaine de
minutes de finalisation avant d'ouvrir quoi que ce soit. Ce n'est pas un défaut
du point de retour : c'est ce que le disque contenait à ce moment-là, et l'avoir
appris justifie le second. On repart de `prete`.

## Les pièges, chacun payé une fois

**`if=virtio` rend le disque invisible.** Windows Setup n'embarque aucun pilote
virtio : l'écran de sélection affiche une liste vide, le fichier de réponses ne
trouve pas de disque 0, et l'installation s'arrête sans un mot ni un octet
écrit. Vue de l'extérieur, la machine paraît seulement lente. Le disque est donc
en **SATA** sur le contrôleur AHCI de la machine q35, reconnu nativement.

**`pkill -f "swtpm socket"` correspond à sa propre ligne de commande** et tue le
shell qui l'exécute, avant tout affichage. Trois commandes de suite ont rendu un
silence total, sans erreur ni trace. D'où les motifs `"[s]wtpm socket"` dans le
script.

**Git Bash traduit `/srv/lab/...` en `C:/Program Files/Git/srv/...`.** Le fichier
part ailleurs et l'exécution échoue sur un chemin inexistant. `MSYS_NO_PATHCONV=1`
depuis l'hôte.

**Un fichier de réponses mal formé ne produit aucune erreur** : Setup l'ignore et
retombe en interactif, et l'on croit à un problème de support. `xmllint --noout`
avant usage.

**« Progression de la mise à jour : 100 %. N'éteignez pas votre ordinateur. »**
Cet écran veut dire ce qu'il dit. Trois arrêts successifs, à cinq puis vingt
minutes, ont chacun renvoyé la finalisation à son début, si bien qu'aucune
session ne s'ouvrait jamais et que la machine paraissait bloquée. Elle ne l'était
pas : c'est l'observateur qui la remettait à zéro. On attend, ou l'on ne démarre
pas.

**`pgrep -x qemu-system-x86_64` ne trouve jamais rien.** Le noyau tronque le nom
de processus à quinze caractères, et celui-ci en fait dix-huit : la recherche est
structurellement incapable de répondre oui. La boucle qui l'employait a donc
annoncé « arrêt propre » au bout de dix secondes sur une machine encore allumée.
`pgrep -f` compare la ligne de commande complète, et s'exclut lui-même,
contrairement à `ps | grep`. Et l'on éprouve le détecteur **avant** de le croire :
`vivante || refus` sur une VM que l'on sait allumée.

**`sendkey` émet des scancodes US, l'invité est en AZERTY.** Frapper
`Keystone!Lab1` touche par touche produit `Keystone/Lqb&` : `a` se trouve à la
position US `q`, `!` à la position US `/`, et les chiffres exigent la touche
majuscule. Rien ne le signale, l'écran de verrouillage affiche des points, et
l'on conclut à un mot de passe erroné. La traduction est dans
`scripts/labo/ouvrir.sh`.

**Un écran noir ne veut pas dire « bloqué ».** Windows éteint l'affichage après
quelques minutes d'inactivité, y compris session ouverte. Une touche envoyée par
le moniteur QEMU tranche en six secondes, là où une capture d'écran ne tranche
rien.

**Le dossier de démarrage de l'utilisateur est à dix niveaux de profondeur.**
`Users/lab/AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Startup` : un
`find -maxdepth 9` s'arrête juste au-dessus et rend une liste vide, qui se lit
comme une absence. Le lanceur y était pourtant depuis le début.

**`cmd.exe /c "…"` lancé depuis Git Bash bascule en session interactive.** La
commande n'est jamais exécutée : `cmd` ouvre une invite, affiche sa bannière de
version, et attend une saisie qui ne viendra pas. Vu de l'extérieur, l'appel ne
rend rien puis expire, ce qui se lit comme un refus d'accès ou comme un outil qui
ne répond plus. En cause, la traduction de chemins de MSYS, qui atteint
l'argument `/c` avant que `cmd` ne le reçoive. Deux parades : `cmd //c "…"`, où le
double slash protège l'argument, ou un fichier `.bat` déposé puis appelé par son
chemin Windows, forme préférable dès qu'il y a des guillemets ou des
redirections. Rencontré en reproduisant un relevé de `reg export` pour
l'[ADR-0021](../../docs/adr/0021-ce-quun-instantane-sait-defaire.md), et payé le
même jour par deux personnes qui ont chacune conclu, à tort, à un refus de
`reg.exe`.

## Surveiller sans se mentir

Quatre détecteurs ont été écrits avant d'en avoir deux justes, et les trois
manqués se sont trompés de la même façon : en concluant depuis un signal supposé
plutôt que mesuré.

| Critère | Ce qu'il a donné |
|---|---|
| « le disque ne grossit plus » | déclenché à **54 %** — l'installateur fait des pauses pendant l'expansion des fichiers |
| « la couleur n'est plus le bleu `0 120 215` » | déclenché au **premier relevé** — le bleu réel est `0 90 158`, la valeur avait été écrite de mémoire |
| « la couleur n'est plus celle **mesurée au démarrage** » | correct |
| `pgrep -x qemu-system-x86_64` | **toujours faux** — nom tronqué à quinze caractères, donc jamais de correspondance possible |
| `pgrep -f qemu-system-x86_64`, **éprouvé sur une VM allumée avant d'être cru** | correct |

Un détecteur ne se cale jamais sur une valeur supposée, et une barrière qu'on n'a
pas essayé de franchir ne prouve rien. C'est la règle que ce dépôt applique à ses
barrières de code ; elle vaut aussi pour l'outillage qui les observe, et les deux
détecteurs justes de ce tableau sont précisément les deux qui ont été confrontés
au cas positif avant de servir.

Corollaire, appris le même jour : une capture d'écran montre un bureau, elle ne
prouve pas qu'une session s'est ouverte. La preuve est l'horodatage de
`Users/<nom>/NTUSER.DAT`, qui se lit disque démonté.

## Le mot de passe

`lab` / `Keystone!Lab1`, en clair dans le fichier de réponses. C'est **voulu** :
cette machine est jetable, sans donnée, sans réseau vers l'extérieur, et destinée
à être détruite après chaque essai. Un secret dans un labo qu'on recrée en une
commande n'est pas un secret — mais il ne doit jamais sortir d'ici.

## Détruire

```powershell
wsl --unregister ks-lab
Remove-Item C:\wsl\ks-lab -Recurse -Force
```
