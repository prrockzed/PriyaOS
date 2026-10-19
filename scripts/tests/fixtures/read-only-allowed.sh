# Lines a read-only survey legitimately uses; none may be flagged.
findmnt -no FSTYPE / 2>/dev/null
mountpoint -q /boot 2>/dev/null
pacman -Si quickshell >/dev/null 2>&1
pacman -Ss hyprland
pacman -Q hyprland
swapon --show=NAME,TYPE,SIZE --noheadings
systemctl is-active NetworkManager
systemctl --user is-active pipewire
echo "error" >&2
awk 'NR > 1 {print $1}'
sed -e "s/\\b${u}\\b/<user>/g"
awk '/proc/self/mountinfo/'
echo "Mounted btrfs subvolumes; unmounted ones need root"
hyprctl -j binds
echo done # rm -rf / is only a comment here
