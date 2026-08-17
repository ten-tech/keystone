#!/bin/bash
# Ouvre une session dans le labo, sans attente calee sur une duree supposee.
#
# Deux pieges que ce script ferme, chacun paye une fois :
#
# 1. `sendkey` emet des scancodes de disposition US, l'invite est en AZERTY.
#    « Keystone!Lab1 » frappe naivement donne « Keystone/Lqb& » : 'a' est a la
#    position US 'q', '!' a la position US '/', et les chiffres exigent Maj.
# 2. Attendre 90 s puis frapper echoue quand la machine n'est pas encore a
#    l'ecran de verrouillage. On attend donc que l'ecran soit CLAIR, ce qui se
#    mesure : un ecran eteint ou un ecran de demarrage rend une luminosite
#    moyenne proche de zero, le fond d'ecran de verrouillage largement au-dessus.
#    Le script imprime la valeur relevee a chaque essai — un seuil qu'on ne voit
#    pas est un seuil qu'on ne peut pas corriger.
set -u
M=/srv/lab/vm/mon.sock
t() { printf "sendkey %s\n" "$1" | timeout 10 socat - UNIX-CONNECT:"$M" >/dev/null 2>&1; sleep 0.25; }
luminosite() {
  printf 'screendump /srv/lab/vm/l.ppm\n' | timeout 25 socat - UNIX-CONNECT:"$M" >/dev/null 2>&1
  sleep 1
  ppmtopgm /srv/lab/vm/l.ppm 2>/dev/null | pgmhist -machine 2>/dev/null \
    | awk '{s+=$1*$2; n+=$2} END {if(n>0) printf "%d", s/n; else print 0}'
}

clair=0
for i in $(seq 1 40); do
  sleep 15
  m=$(luminosite)
  echo "  essai $i : luminosite ${m:-0}"
  # Un ecran noir mesure 0 a 3 ; le fond de verrouillage de Windows 11 mesure
  # plus de 60. Le seuil est donc largement separe des deux cas observes.
  if [ "${m:-0}" -gt 25 ]; then clair=1; break; fi
  # Un ecran eteint se reveille a la moindre touche ; l'envoyer ne coute rien
  # et evite d'attendre un ecran qui ne se rallumera pas tout seul.
  t shift
done
[ "$clair" = 1 ] || { echo "REFUS : l ecran n est jamais devenu clair"; exit 1; }

t ret; sleep 4
for k in shift-k e y s t o n e slash shift-l q b shift-1; do t "$k"; done
t ret
echo "mot de passe envoye"
