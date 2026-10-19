#!/usr/bin/env bash
# PriyaOS machine survey (sub-phase 0.1).
#
# Prints read-only facts about this computer as Markdown: hardware, disks and
# boot, desktop, and the software PriyaOS builds on. It never changes anything
# and refuses to run as root. User name, host name, home path and disk UUIDs
# are masked, and serial numbers and MAC addresses are never read. The output
# still describes your machine, so keep it private: share it with whoever is
# helping you, but don't commit it to a public repository.
#
# Usage: bash scripts/survey.sh > survey.md
#
# The same checks will later feed the installer and `priya doctor`, so keep
# them general: never assume a particular laptop.

set -u

SURVEY_VERSION=2

have() { command -v "$1" >/dev/null 2>&1; }
# Bounded run, so a stuck daemon can't hang the survey.
run() { timeout 5 "$@" 2>/dev/null; }
section() { printf '\n## %s\n\n' "$1"; }
cell() { local v="${1:-}"; v="${v//|/\\|}"; printf '%s' "${v:-—}"; }
row() { printf '| %s | %s |\n' "$1" "$(cell "${2:-}")"; }
codeblock() { printf '```text\n'; cat; printf '```\n'; }
names() { find "$1" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null | sort | paste -sd, -; }
# pacman -Q also answers for a name another package provides (rust -> rustup),
# so say which package that is instead of reporting the name as installed.
pkgver() { pacman -Q "$1" 2>/dev/null | awk -v p="$1" 'NR == 1 { if ($1 == p) print $2; else print "provided by " $1 }'; }

# --- Facts used in more than one place -------------------------------------

dmi() { cat "/sys/class/dmi/id/$1" 2>/dev/null; }

bootloaders() {
	# The EFI folder is often root-only, so also look for each loader's files
	# outside it. Windows Boot Manager is listed when present: it matters for
	# dual boot.
	local e=${esp:-/nonexistent} found=()
	if [ -d "$e/EFI/GRUB" ] || [ -d "$e/EFI/grub" ] || [ -d "$e/EFI/arch" ] || [ -e /boot/grub/grub.cfg ]; then
		found+=("GRUB")
	fi
	if [ -d "$e/EFI/systemd" ] || [ -d "$e/loader/entries" ] || [ -d /boot/loader/entries ]; then
		found+=("systemd-boot")
	fi
	if [ -d "$e/EFI/limine" ] || [ -e "$e/limine.conf" ] || [ -e /boot/limine.conf ]; then
		found+=("Limine")
	fi
	if [ -d "$e/EFI/refind" ] || [ -e /boot/refind_linux.conf ]; then
		found+=("rEFInd")
	fi
	if [ -d "$e/EFI/Microsoft" ]; then
		found+=("Windows Boot Manager")
	fi
	local IFS=,
	local list="${found[*]}"
	echo "${list//,/, }"
}

secure_boot() {
	local f=/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c
	[ -d /sys/firmware/efi ] || { echo "n/a (legacy BIOS boot)"; return; }
	[ -r "$f" ] || { echo "unknown"; return; }
	# The 5th byte is the value; the first 4 are attributes.
	case $(od -An -t u1 -j4 -N1 "$f" | tr -d ' ') in
	1) echo "on" ;;
	0) echo "off" ;;
	*) echo "unknown" ;;
	esac
}

gather_facts() {
	model="$(dmi sys_vendor) $(dmi product_name)"
	model="${model# }"

	root_fs=$(findmnt -no FSTYPE / 2>/dev/null)
	root_src=$(findmnt -no SOURCE / 2>/dev/null)
	root_opts=$(findmnt -no OPTIONS / 2>/dev/null)

	# The ESP is the vfat filesystem mounted at one of the usual places.
	esp=""
	for p in /efi /boot/efi /boot; do
		if [ "$(findmnt -no FSTYPE "$p" 2>/dev/null)" = "vfat" ]; then
			esp=$p
			break
		fi
	done
	esp_size=""
	[ -n "$esp" ] && esp_size=$(df -h --output=size "$esp" 2>/dev/null | tail -1 | tr -d ' ')

	bootloader=$(bootloaders)

	fstypes=$(lsblk -rno FSTYPE 2>/dev/null)
	if grep -qx BitLocker <<<"$fstypes"; then
		windows="yes, encrypted with BitLocker"
	elif grep -qx ntfs <<<"$fstypes"; then
		windows="yes, not encrypted"
	else
		windows="none found"
	fi
	if grep -qx crypto_LUKS <<<"$fstypes"; then luks="yes"; else luks="no"; fi

	in_hyprland=no
	[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ] && have hyprctl && in_hyprland=yes
}

survey() {
	printf '# Machine survey\n\n'
	# shellcheck disable=SC2016 # backticks are Markdown, not shell
	printf 'Made by `scripts/survey.sh` v%s on %s. Read-only. User and host names, home paths and disk IDs are masked.\n' \
		"$SURVEY_VERSION" "$(date -u +%Y-%m-%d)"

	section "Summary"
	echo "| Question | Answer |"
	echo "|---|---|"
	row "Computer" "$model"
	row "Arch disk format" "$root_fs"
	row "Startup menu program" "${bootloader:-unknown}"
	row "EFI partition" "${esp:+$esp, $esp_size}"
	row "Windows on this disk" "$windows"
	row "Linux disk encryption (LUKS)" "$luks"
	row "Secure Boot" "$(secure_boot)"
	row "Desktop" "$(if [ $in_hyprland = yes ]; then run hyprctl version | head -1 | cut -d' ' -f1,2; else echo "${XDG_CURRENT_DESKTOP:-unknown (not run inside a desktop session)}"; fi)"

	section "Hardware"
	echo "| Item | Value |"
	echo "|---|---|"
	row "Vendor / model" "$model"
	row "Product family" "$(dmi product_family)"
	row "Board" "$(dmi board_name)"
	row "Firmware (BIOS) version" "$(dmi bios_version)"
	row "CPU" "$(lscpu 2>/dev/null | sed -n 's/^Model name:[[:space:]]*//p' | head -1)"
	row "CPU threads" "$(nproc)"
	row "RAM" "$(awk '/^MemTotal/ {printf "%.1f GiB", $2/1048576}' /proc/meminfo)"
	row "Swap" "$(swapon --show=NAME,TYPE,SIZE --noheadings 2>/dev/null | awk '{print $1" ("$2", "$3")"}' | paste -sd, -)"
	row "Kernel" "$(uname -r)"
	row "Distribution" "$(sed -n 's/^PRETTY_NAME=//p' /etc/os-release 2>/dev/null | tr -d '"')"
	row "Boot mode" "$(if [ -d /sys/firmware/efi ]; then echo UEFI; else echo "legacy BIOS"; fi)"
	if have lspci; then
		echo
		echo "PCI devices that matter (graphics, network, audio) with their drivers:"
		echo
		lspci -k 2>/dev/null | awk '
			/^[0-9a-f]/ { keep = ($0 ~ /VGA|3D controller|Display controller|Network controller|Ethernet controller|Audio device|Multimedia audio/) }
			keep && (/^[0-9a-f]/ || /Kernel driver in use/)' | codeblock
	else
		echo
		echo "_lspci not installed (package pciutils), so graphics and Wi-Fi details are missing._"
	fi

	section "Disks and boot"
	lsblk -o NAME,SIZE,TYPE,FSTYPE,FSVER,PARTTYPENAME,MOUNTPOINTS 2>/dev/null | codeblock
	echo
	echo "| Item | Value |"
	echo "|---|---|"
	row "Root filesystem" "$root_fs on $root_src"
	row "Root filesystem options" "$root_opts"
	row "Kernel command line" "$(cat /proc/cmdline 2>/dev/null)"
	row "fstab uses subvolid=" "$(if grep -Eq '^[^#].*subvolid=' /etc/fstab 2>/dev/null; then echo yes; else echo no; fi)"
	row "fstab / entry" "$(awk '$2 == "/" && $1 !~ /^#/' /etc/fstab 2>/dev/null)"
	row "/boot" "$(if mountpoint -q /boot 2>/dev/null; then findmnt -no FSTYPE,SOURCE /boot; else echo "inside the root filesystem"; fi)"
	row "EFI partition" "${esp:-not found}${esp:+ ($esp_size)}"
	row "EFI folder readable" "$(if [ -n "$esp" ] && [ -r "$esp/EFI" ]; then names "$esp/EFI"; elif [ -n "$esp" ]; then echo "no (needs root; that's fine)"; else echo "—"; fi)"
	row "Startup menu program" "${bootloader:-unknown}"
	row "Windows" "$windows"
	row "Free space on /" "$(df -h --output=avail / 2>/dev/null | tail -1 | tr -d ' ')"
	row "Free space on /home" "$(df -h --output=avail /home 2>/dev/null | tail -1 | tr -d ' ')"

	if [ "$root_fs" = "btrfs" ]; then
		echo
		echo "Mounted btrfs subvolumes (from /proc/self/mountinfo; unmounted ones need root to list):"
		echo
		echo "| Mount point | Subvolume |"
		echo "|---|---|"
		awk '{ for (i = 7; i <= NF; i++) if ($i == "-") { if ($(i+1) == "btrfs") printf "| %s | %s |\n", $5, $4; break } }' /proc/self/mountinfo
	fi

	section "Desktop"
	echo "| Item | Value |"
	echo "|---|---|"
	row "Session type" "${XDG_SESSION_TYPE:-unknown}"
	row "Desktop name" "${XDG_CURRENT_DESKTOP:-unknown}"
	row "Hyprland" "$(if [ $in_hyprland = yes ]; then run hyprctl version | head -1; elif have Hyprland || have hyprctl; then echo "installed, but this terminal isn't inside Hyprland"; else echo "not installed"; fi)"
	row "Omarchy" "$(if [ -d "$HOME/.local/share/omarchy" ]; then echo yes; else echo no; fi)"
	row "Login manager" "$(basename "$(readlink /etc/systemd/system/display-manager.service 2>/dev/null)" .service 2>/dev/null)"
	row "Bars and shells running" "$(for p in waybar quickshell qs ags eww hyprpanel ironbar nwg-panel; do pgrep -x "$p" >/dev/null && echo "$p"; done | paste -sd, -)"
	row "Launchers running" "$(for p in walker wofi rofi fuzzel tofi anyrun; do pgrep -x "$p" >/dev/null && echo "$p"; done | paste -sd, -)"
	row "Notification daemon" "$(run busctl --user status org.freedesktop.Notifications | sed -n 's/^Comm=//p')"
	row "Quickshell configs" "$(if [ -d "$HOME/.config/quickshell" ]; then names "$HOME/.config/quickshell"; else echo none; fi)"
	row "Keyboard layout" "$(run localectl status | sed -n 's/^ *X11 Layout: //p')"
	row "Language" "${LANG:-unset}"

	if [ $in_hyprland = yes ] && have python3; then
		echo
		echo "Monitors:"
		echo
		echo "| Output | Panel | Mode | Scale | Rotation |"
		echo "|---|---|---|---|---|"
		run hyprctl -j monitors | python3 -c '
import json, sys
for m in json.load(sys.stdin):
    print("| %s | %s %s | %dx%d @ %.0f Hz | %s | %s |" % (
        m.get("name"), m.get("make", ""), m.get("model", ""),
        m.get("width", 0), m.get("height", 0), m.get("refreshRate", 0),
        m.get("scale"), m.get("transform")))
'
		echo
		echo "Keys PriyaOS plans to use, and what they do now:"
		echo
		echo "| Key | Bound to now |"
		echo "|---|---|"
		run hyprctl -j binds | python3 -c '
import json, sys
SUPER, SHIFT = 64, 1
want = [((SUPER, "space"), "Super+Space (Priya bar)"),
        ((SUPER, "j"), "Super+J (Task center)"),
        ((SUPER, "p"), "Super+P (agent workspace)"),
        ((SUPER, "v"), "Super+V (push-to-talk)"),
        ((SUPER | SHIFT, "s"), "Super+Shift+S (Priya Vision)"),
        ((SUPER | SHIFT, "escape"), "Super+Shift+Escape (kill switch)")]
found = {}
for b in json.load(sys.stdin):
    if b.get("submap"):
        continue
    key = (b.get("modmask"), str(b.get("key", "")).lower())
    found.setdefault(key, []).append((b.get("dispatcher", "") + " " + b.get("arg", "")).strip())
for key, label in want:
    now = "; ".join(found.get(key, [])) or "free"
    print("| %s | %s |" % (label, now.replace("|", "\\|")))
'
	elif [ $in_hyprland = yes ]; then
		echo
		echo "_python3 is needed to read monitors and key bindings._"
	else
		echo
		echo "_Run the survey from a terminal inside Hyprland to include monitors and key bindings._"
	fi

	section "Software"
	echo "| Package or service | Status |"
	echo "|---|---|"
	for p in hyprland quickshell qt6-base qt6-declarative python git base-devel rust rustup ollama sof-firmware pipewire wireplumber networkmanager iwd gnome-keyring kwallet keepassxc linux linux-lts linux-zen; do
		v=$(pkgver "$p")
		if [ -n "$v" ]; then
			row "$p" "$v"
		elif [ "$p" = quickshell ] || [ "$p" = ollama ]; then
			# Not installed: say whether the official repos have it.
			repo=$(pacman -Si "$p" 2>/dev/null | awk -F': ' '/^Repository/ {r=$2} /^Version/ {v=$2} END {if (r) print r" "v}')
			row "$p" "not installed${repo:+ (available in $repo)}"
		else
			row "$p" "not installed"
		fi
	done
	row "AUR helper" "$(for h in paru yay; do have "$h" && echo "$h"; done | paste -sd, -)"
	row "Secret Service (keyring)" "$(run busctl --user list | awk '$1 == "org.freedesktop.secrets" {print $3}' | head -1)"
	row "Network service" "$(for s in NetworkManager iwd systemd-networkd; do [ "$(systemctl is-active "$s" 2>/dev/null)" = active ] && echo "$s"; done | paste -sd, -)"
	row "Audio service" "$(for s in pipewire wireplumber pulseaudio; do [ "$(systemctl --user is-active "$s" 2>/dev/null)" = active ] && echo "$s"; done | paste -sd, -)"
	row "Python" "$(python3 --version 2>/dev/null)"
	row "Rust" "$(rustc --version 2>/dev/null)"
	row "Ollama service" "$(systemctl is-active ollama 2>/dev/null)"
	row "Ollama models" "$(have ollama && run ollama list | awk 'NR > 1 {print $1}' | paste -sd, -)"

	section "Only you can check"
	echo "- In Windows: if \"Windows on this disk\" says BitLocker, make sure the recovery key is saved somewhere off this laptop (account.microsoft.com/devices/recoverykey). Needed before Phase 2 changes boot settings."
}

# Mask anything personal before it reaches the screen.
mask() {
	local u h
	u=$(id -un)
	h=$(cat /proc/sys/kernel/hostname 2>/dev/null)
	local args=(-E -e "s#${HOME}#~#g" -e "s#/home/${u}#~#g"
		-e 's/[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}/<uuid>/g'
		-e 's/\b[0-9A-F]{4}-[0-9A-F]{4}\b/<id>/g')
	[ ${#u} -ge 3 ] && args+=(-e "s/\\b${u}\\b/<user>/g")
	[ ${#h} -ge 3 ] && args+=(-e "s/\\b${h}\\b/<host>/g")
	sed "${args[@]}"
}

main() {
	if [ "$(id -u)" -eq 0 ]; then
		echo "survey.sh: run this as your normal user, not root. It only reads." >&2
		exit 1
	fi
	if [ "$(uname -s)" != "Linux" ]; then
		echo "survey.sh: this survey is for Linux (Arch). Nothing to do on $(uname -s)." >&2
		exit 1
	fi
	gather_facts
	survey | mask
}

# When sourced (by tests) only the functions are defined; nothing runs.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
	main "$@"
fi
