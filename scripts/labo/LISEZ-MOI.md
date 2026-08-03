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

## Les quatre pièges, chacun payé une fois

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

## Surveiller sans se mentir

Trois détecteurs ont été écrits avant d'en avoir un juste, et les deux premiers
se sont trompés de la même façon — en concluant depuis un signal supposé plutôt
que mesuré.

| Critère | Ce qu'il a donné |
|---|---|
| « le disque ne grossit plus » | déclenché à **54 %** — l'installateur fait des pauses pendant l'expansion des fichiers |
| « la couleur n'est plus le bleu `0 120 215` » | déclenché au **premier relevé** — le bleu réel est `0 90 158`, la valeur avait été écrite de mémoire |
| « la couleur n'est plus celle **mesurée au démarrage** » | correct |

Un détecteur ne se cale jamais sur une valeur supposée. C'est la règle que ce
dépôt applique à ses barrières de code ; elle vaut aussi pour l'outillage qui
les observe.

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
