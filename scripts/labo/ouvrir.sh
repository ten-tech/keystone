#!/bin/bash
M=/srv/lab/vm/mon.sock
t() { printf "sendkey %s\n" "$1" | timeout 10 socat - UNIX-CONNECT:"$M" >/dev/null 2>&1; sleep 0.25; }
t ret; sleep 3
# « Keystone!Lab1 » frappe sur un clavier AZERTY, alors que sendkey emet des
# scancodes de disposition US. Les touches qui DIFFERENT doivent etre traduites :
#   'a' se trouve a la position US 'q' ; '!' a la position US '/' ;
#   les chiffres exigent Maj, la rangee non modifiee donnant &é"'(
# Frapper naivement produirait « Keystone/Lqb& », qu'aucun test ne verrait.
for k in shift-k e y s t o n e slash shift-l q b shift-1; do t "$k"; done
t ret
sleep 45
printf 'screendump /srv/lab/vm/session.ppm\n' | timeout 25 socat - UNIX-CONNECT:"$M" >/dev/null 2>&1
sleep 2
pnmtopng /srv/lab/vm/session.ppm > /mnt/c/Users/pc/AppData/Local/Temp/labo-session.png 2>/dev/null && echo ok
