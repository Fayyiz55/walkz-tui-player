#!/usr/bin/env bash
# ♫ Walkz TUI one-shot installer.
#
#   ./install.sh            install everything (asks sudo password once)
#   ./install.sh --dry-run  show the plan, touch nothing
#
# Installs the compiled app onto your PATH together with its runtime
# friends: FFmpeg (ffplay — audio playback) and cava (spectrum visualizer).
# Missing pieces are installed through your package manager
# (apt / dnf / pacman / brew) — nothing else is downloaded or bundled.

set -euo pipefail

DRY_RUN=0
for arg in "$@"; do
	case "$arg" in
	--dry-run | -n) DRY_RUN=1 ;;
	-h | --help)
		grep '^#' "$0" | sed 's/^# \{0,1\}//'
		exit 0
		;;
	*)
		echo "usage: ./install.sh [--dry-run]" >&2
		exit 1
		;;
	esac
done

say() { printf '%s\n' "$*"; }
bold() { printf '\033[1m%s\033[0m\n' "$*"; }
ok() { printf '  \033[32m✔\033[0m %s\n' "$*"; }
bad() { printf '  \033[31m✘\033[0m %s\n' "$*"; }
skip() { printf '  \033[33m○\033[0m %s\n' "$*"; }

have() { command -v "$1" >/dev/null 2>&1; }

# Go to the repo root (this script lives there).
cd "$(dirname "${BASH_SOURCE[0]}")"

if [[ $DRY_RUN -eq 1 ]]; then
	bold "♫ Walkz TUI installer — DRY RUN (nothing will be changed)"
	skip "a real run installs: FFmpeg, cava, the Rust build, then 'walkz-tui' on your PATH"
else
	bold "♫ Walkz TUI installer"
	say "This installs FFmpeg + cava through your package manager, builds the app,"
	say "and puts 'walkz-tui' on your PATH. sudo may ask for your password."
	say ""
fi

MISSING=()

# --- Package manager -----------------------------------------------------
detect_pm() {
	for pm in apt-get dnf pacman brew; do
		if have "$pm"; then
			echo "$pm"
			return 0
		fi
	done
	return 1
}

# Package name providing $1 (a binary) under $2 (a package manager).
pkg_for() {
	local cap="$1" pm="$2"
	case "$cap:$pm" in
	ffplay:apt) echo "ffmpeg" ;;
	ffplay:dnf) echo "ffmpeg" ;;
	ffplay:pacman) echo "ffmpeg" ;;
	ffplay:brew) echo "ffmpeg" ;;
	cava:*) echo "cava" ;;
	cargo:apt) echo "cargo" ;;
	cargo:dnf) echo "cargo" ;;
	cargo:pacman) echo "rust" ;;
	cargo:brew) echo "rust" ;;
	*) return 1 ;;
	esac
}

# One install command per manager ($pm, $pkg set by caller).
pm_install_cmd() {
	case "$1" in
	apt-get) echo "sudo apt-get install -y $2" ;;
	dnf) echo "sudo dnf install -y $2" ;;
	pacman) echo "sudo pacman -S --noconfirm $2" ;;
	brew) echo "brew install $2" ;;
	esac
}

pm_do_install() {
	local pm="$1" pkg="$2"
	case "$pm" in
	apt-get)
		sudo apt-get install -y "$pkg" ||
			{ say "  refreshing package index..."; sudo apt-get update && sudo apt-get install -y "$pkg"; }
		;;
	dnf) sudo dnf install -y "$pkg" ;;
	pacman) sudo pacman -S --noconfirm "$pkg" ;;
	brew) brew install "$pkg" ;;
	esac
}

# ensure <binary> <human-label> — install through the package manager when missing.
ensure() {
	local cap="$1" label="$2" pm pkg
	if have "$cap"; then
		ok "$cap found — $label"
		return 0
	fi
	if ! pm="$(detect_pm)"; then
		bad "$cap missing — $label"
		skip "no apt/dnf/pacman/brew found; install $cap manually"
		MISSING+=("$cap")
		return 1
	fi
	pkg="$(pkg_for "$cap" "$pm")" || {
		bad "$cap missing — $label"
		skip "no package recipe for $pm; install $cap manually"
		MISSING+=("$cap")
		return 1
	}
	if [[ $DRY_RUN -eq 1 ]]; then
		printf '  [dry-run] %s\n' "$(pm_install_cmd "$pm" "$pkg")"
		return 0
	fi
	bold "Installing $pkg (provides $cap)..."
	pm_do_install "$pm" "$pkg"
	have "$cap" && ok "$cap installed" || {
		bad "$cap still missing after installing $pkg"
		MISSING+=("$cap")
		return 1
	}
}

bold "Runtime friends"
ensure ffplay "audio playback (Never pins a version; any FFmpeg works)" || true
ensure cava "live spectrum visualizer" || true
ensure cargo "builds the app" || true

# --- Build ----------------------------------------------------------------
bold "Building Walkz (release)"
if [[ $DRY_RUN -eq 1 ]]; then
	printf '  [dry-run] cargo build --release\n'
else
	cargo build --release
	ok "built target/release/walkz-tui"
fi

# --- PATH install -----------------------------------------------------------
warn_slow_path() {
	bad "~/.cargo/bin not on your PATH?"
	skip "add it:  echo 'export PATH=\"$HOME/.cargo/bin:\$PATH\"' >> ~/.bashrc && source ~/.bashrc"
	MISSING+=("PATH")
}

bold "Installing on your PATH"
if [[ $DRY_RUN -eq 1 ]]; then
	printf '  [dry-run] cargo install --path . (-> ~/.cargo/bin/walkz-tui)\n'
else
	cargo install --path . --quiet
	have walkz-tui && ok "walkz-tui on your PATH" || warn_slow_path
fi

# --- Summary ------------------------------------------------------------------
bold ""
bold "Summary"
have ffplay && ok "ffplay — audio playback ready" || bad "ffplay — playback won't start"
have cava && ok "cava — spectrum lane will dance on Now Playing" || bad "cava — spectrum lane shows 'needs cava'"
if [[ $DRY_RUN -eq 1 ]]; then
	skip "dry run complete — nothing was changed. Re-run without --dry-run to install."
elif [[ ${#MISSING[@]} -gt 0 ]]; then
	bad "finished with unresolved bits: ${MISSING[*]}"
	skip "the app still runs; fix the bits above whenever you like"
else
	ok "everything installed. Run 'walkz-tui' from anywhere — enjoy ♫"
fi
