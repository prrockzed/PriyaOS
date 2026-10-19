# Every non-comment line below must be flagged by check-read-only.sh.
sudo pacman -Syu
mkfs.btrfs /dev/sda1
dd if=/dev/zero of=/dev/sda bs=1M
wipefs -a /dev/sda
btrfs subvolume snapshot / /snap
btrfs subvolume delete /@
mount /dev/sda1 /mnt
x=1; mount -o remount,rw /
systemctl enable --now ollama
systemctl --user start priyad
pacman -S ollama
rm -rf /tmp/x
echo hi > /etc/motd
echo hi >> "$HOME/.bashrc"
sed -i 's/a/b/' /etc/fstab
swapon /dev/zram0
grub-install --target=x86_64-efi
efibootmgr -o 0001
hyprctl keyword bind SUPER,space,exec,x
cp a b
