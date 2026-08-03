#!/bin/bash
# Labo Keystone — installation non interactive de Windows 11 Pro.
#
# LE DISQUE EST EN SATA, PAS EN VIRTIO, ET C'EST DELIBERE.
# Windows Setup n'embarque aucun pilote virtio : avec `if=virtio`, l'ecran
# « Selectionner l'emplacement d'installation » affiche une liste VIDE, le
# fichier de reponses ne trouve pas de disque 0, et l'installation s'arrete
# la sans rien ecrire ni rien dire. Le controleur AHCI de la machine q35 est
# reconnu nativement. On perd un peu de debit, on gagne de fonctionner.
set -u
pkill -f "[k]s-lab.qcow2" 2>/dev/null || true
pkill -f "[s]wtpm socket" 2>/dev/null || true
sleep 2
rm -f /srv/lab/vm/ks-lab.qcow2 /srv/lab/vm/mon.sock
qemu-img create -f qcow2 /srv/lab/vm/ks-lab.qcow2 64G > /dev/null
cp /usr/share/OVMF/OVMF_VARS_4M.fd /srv/lab/vm/vars.fd
swtpm socket --tpm2 --tpmstate dir=/srv/lab/tpm --ctrl type=unixio,path=/srv/lab/tpm/sock --daemon
sleep 2
qemu-system-x86_64 -daemonize \
  -accel kvm -cpu host -smp 4 -m 4096 -machine q35,smm=on \
  -drive if=pflash,format=raw,unit=0,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.secboot.fd \
  -drive if=pflash,format=raw,unit=1,file=/srv/lab/vm/vars.fd \
  -chardev socket,id=chrtpm,path=/srv/lab/tpm/sock \
  -tpmdev emulator,id=tpm0,chardev=chrtpm -device tpm-crb,tpmdev=tpm0 \
  -drive file=/srv/lab/vm/ks-lab.qcow2,if=none,id=hd0,format=qcow2 \
  -device ide-hd,drive=hd0,bus=ide.0,bootindex=2 \
  -drive file=/srv/lab/vm/win11.iso,if=none,id=cd0,media=cdrom,readonly=on \
  -device ide-cd,drive=cd0,bus=ide.1,bootindex=1 \
  -drive file=/srv/lab/vm/unattend.iso,if=none,id=cd1,media=cdrom,readonly=on \
  -device ide-cd,drive=cd1,bus=ide.2 \
  -vga std -display none -netdev user,id=n0 -device e1000,netdev=n0 \
  -monitor unix:/srv/lab/vm/mon.sock,server,nowait
echo "labo relance, disque en SATA"
