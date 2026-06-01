#!/usr/bin/env bash
# =============================================================================
# apply_patches.sh
# Applies unified diff (.patch) files to source code using Python's difflib/
# patch logic. Handles multiline replacements cleanly via an embedded Python
# patcher script written with cat-EOF.
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Config — override via env or edit here
# ---------------------------------------------------------------------------
PATCH_DIR="${PATCH_DIR:-./patches}"      # Directory containing .patch files
TARGET_DIR="${TARGET_DIR:-.}"            # Root of the source tree to patch
BACKUP_EXT="${BACKUP_EXT:-.orig}"        # Extension for backup files ("" = no backups)
DRY_RUN="${DRY_RUN:-0}"                  # Set to 1 to simulate without writing

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}[INFO]${RESET}  $*"; }
ok()      { echo -e "${GREEN}[OK]${RESET}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${RESET}  $*"; }
error()   { echo -e "${RED}[ERROR]${RESET} $*" >&2; }
die()     { error "$*"; exit 1; }

# ---------------------------------------------------------------------------
# Write the Python patcher to a temp file using cat-EOF
# Python handles multiline hunks far more cleanly than sed/awk.
# ---------------------------------------------------------------------------
PATCHER_PY="$(mktemp /tmp/patcher_XXXXXX.py)"
trap 'rm -f "$PATCHER_PY"' EXIT

cat > "$PATCHER_PY" << 'PYTHON_SCRIPT'
#!/usr/bin/env python3
"""
patcher.py — applies a unified diff (.patch) to a source tree.

Usage:
    python3 patcher.py <patch_file> <target_dir> [--dry-run] [--backup <ext>]
"""

import sys
import os
import re
import shutil
import argparse
from pathlib import Path


# ── Argument parsing ────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser(description="Apply a unified diff patch.")
    p.add_argument("patch_file", help="Path to the .patch file")
    p.add_argument("target_dir", help="Root directory of the source tree")
    p.add_argument("--dry-run", action="store_true", help="Simulate only")
    p.add_argument("--backup", default="", metavar="EXT",
                   help="Create backups with this extension (e.g. .orig)")
    return p.parse_args()


# ── Patch file parser ────────────────────────────────────────────────────────

class Hunk:
    """One @@ hunk from a unified diff."""
    def __init__(self, old_start, old_count, new_start, new_count):
        self.old_start  = old_start   # 1-based
        self.old_count  = old_count
        self.new_start  = new_start
        self.new_count  = new_count
        self.lines      = []          # list of (op, text); op in {' ','+','-'}

    def __repr__(self):
        return (f"Hunk(old={self.old_start}+{self.old_count}, "
                f"new={self.new_start}+{self.new_count}, "
                f"lines={len(self.lines)})")


class FilePatch:
    """All hunks for a single file."""
    def __init__(self, old_path, new_path):
        self.old_path = old_path
        self.new_path = new_path
        self.hunks: list[Hunk] = []


def parse_patch(patch_text: str) -> list[FilePatch]:
    """Parse unified diff text into a list of FilePatch objects."""
    file_patches: list[FilePatch] = []
    current_fp: FilePatch | None = None
    current_hunk: Hunk | None = None

    HUNK_HEADER = re.compile(
        r'^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@'
    )

    for raw_line in patch_text.splitlines(keepends=True):
        line = raw_line.rstrip('\n').rstrip('\r')

        if line.startswith('--- '):
            old_path = line[4:].split('\t')[0].strip()
            # strip a/ prefix produced by git
            old_path = re.sub(r'^a/', '', old_path)
            current_fp = None
            _pending_old = old_path
            continue

        if line.startswith('+++ '):
            new_path = line[4:].split('\t')[0].strip()
            new_path = re.sub(r'^b/', '', new_path)
            current_fp = FilePatch(_pending_old, new_path)
            file_patches.append(current_fp)
            current_hunk = None
            continue

        if current_fp is None:
            continue

        m = HUNK_HEADER.match(line)
        if m:
            old_start  = int(m.group(1))
            old_count  = int(m.group(2)) if m.group(2) is not None else 1
            new_start  = int(m.group(3))
            new_count  = int(m.group(4)) if m.group(4) is not None else 1
            current_hunk = Hunk(old_start, old_count, new_start, new_count)
            current_fp.hunks.append(current_hunk)
            continue

        if current_hunk is not None:
            if line.startswith('+'):
                current_hunk.lines.append(('+', line[1:]))
            elif line.startswith('-'):
                current_hunk.lines.append(('-', line[1:]))
            elif line.startswith(' ') or line == '':
                current_hunk.lines.append((' ', line[1:] if line else ''))
            # else: "\ No newline at end of file" etc — ignore

    return file_patches


# ── Patch applier ────────────────────────────────────────────────────────────

class PatchError(Exception):
    pass


def apply_hunk(lines: list[str], hunk: Hunk, filename: str) -> list[str]:
    """
    Apply one hunk to `lines` (0-based list, no trailing newlines).
    Returns the modified list.
    """
    result     = lines[:]
    old_lineno = hunk.old_start - 1   # convert to 0-based index
    out_lines  = result[:old_lineno]
    src_pos    = old_lineno

    for op, text in hunk.lines:
        if op == ' ':
            # context line — must match
            if src_pos >= len(result):
                raise PatchError(
                    f"{filename}: hunk context line {src_pos+1} is past EOF"
                )
            actual = result[src_pos]
            if actual != text:
                raise PatchError(
                    f"{filename}: context mismatch at line {src_pos+1}:\n"
                    f"  expected: {text!r}\n"
                    f"  found:    {actual!r}"
                )
            out_lines.append(text)
            src_pos += 1

        elif op == '-':
            # removal — must match
            if src_pos >= len(result):
                raise PatchError(
                    f"{filename}: removal line {src_pos+1} is past EOF"
                )
            actual = result[src_pos]
            if actual != text:
                raise PatchError(
                    f"{filename}: removal mismatch at line {src_pos+1}:\n"
                    f"  expected: {text!r}\n"
                    f"  found:    {actual!r}"
                )
            src_pos += 1   # skip (delete) it

        elif op == '+':
            out_lines.append(text)   # insert new line

    # append remainder of file
    out_lines.extend(result[src_pos:])
    return out_lines


def apply_file_patch(fp: FilePatch, target_dir: Path,
                     dry_run: bool, backup_ext: str) -> bool:
    """Apply all hunks of fp to the target file. Returns True on success."""
    # Resolve target path — use new_path; fall back to old_path
    rel = fp.new_path if fp.new_path != '/dev/null' else fp.old_path
    target = (target_dir / rel).resolve()

    # Deletion patch  (/dev/null as new_path)
    if fp.new_path == '/dev/null':
        if not target.exists():
            print(f"  [SKIP] already absent: {rel}")
            return True
        print(f"  [DELETE] {rel}")
        if not dry_run:
            if backup_ext:
                shutil.copy2(target, str(target) + backup_ext)
            target.unlink()
        return True

    # Creation patch  (/dev/null as old_path)
    if fp.old_path == '/dev/null':
        print(f"  [CREATE] {rel}")
        content = []
        for hunk in fp.hunks:
            for op, text in hunk.lines:
                if op == '+':
                    content.append(text)
        if not dry_run:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text('\n'.join(content) + '\n', encoding='utf-8')
        return True

    # Modification patch
    if not target.exists():
        print(f"  [MISSING] {rel}  — skipping", file=sys.stderr)
        return False

    raw   = target.read_text(encoding='utf-8')
    lines = raw.splitlines()   # no trailing newlines

    try:
        for hunk in fp.hunks:
            lines = apply_hunk(lines, hunk, rel)
    except PatchError as exc:
        print(f"  [FAIL] {exc}", file=sys.stderr)
        return False

    new_content = '\n'.join(lines)
    # preserve trailing newline if original had one
    if raw.endswith('\n'):
        new_content += '\n'

    print(f"  [PATCH] {rel}")
    if not dry_run:
        if backup_ext:
            shutil.copy2(target, str(target) + backup_ext)
        target.write_text(new_content, encoding='utf-8')

    return True


# ── Entry point ──────────────────────────────────────────────────────────────

def main():
    args       = parse_args()
    patch_file = Path(args.patch_file)
    target_dir = Path(args.target_dir).resolve()

    if not patch_file.exists():
        print(f"ERROR: patch file not found: {patch_file}", file=sys.stderr)
        sys.exit(1)

    if not target_dir.is_dir():
        print(f"ERROR: target directory not found: {target_dir}", file=sys.stderr)
        sys.exit(1)

    patch_text   = patch_file.read_text(encoding='utf-8')
    file_patches = parse_patch(patch_text)

    if not file_patches:
        print("WARNING: no file patches found in patch file.", file=sys.stderr)
        sys.exit(0)

    mode_label = " [DRY RUN]" if args.dry_run else ""
    print(f"\nApplying: {patch_file.name}{mode_label}")
    print(f"Target:   {target_dir}\n")

    failed = 0
    for fp in file_patches:
        ok = apply_file_patch(fp, target_dir,
                              dry_run=args.dry_run,
                              backup_ext=args.backup)
        if not ok:
            failed += 1

    print()
    if failed:
        print(f"RESULT: {len(file_patches)-failed} succeeded, "
              f"{failed} failed.", file=sys.stderr)
        sys.exit(1)
    else:
        print(f"RESULT: all {len(file_patches)} file(s) patched successfully.")


if __name__ == "__main__":
    main()
PYTHON_SCRIPT

chmod +x "$PATCHER_PY"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
usage() {
    cat << USAGE
${BOLD}apply_patches.sh${RESET} — apply unified diff patches to source code via Python

${BOLD}USAGE${RESET}
    $0 [OPTIONS] [patch_file_or_dir]

${BOLD}OPTIONS${RESET}
    -t, --target DIR      Source tree root        (default: \$TARGET_DIR or .)
    -p, --patches DIR     Directory of .patch files (default: \$PATCH_DIR or ./patches)
    -b, --backup EXT      Backup extension         (default: \$BACKUP_EXT or .orig)
    -n, --dry-run         Simulate; do not write
    -h, --help            Show this help

${BOLD}ENVIRONMENT${RESET}
    PATCH_DIR, TARGET_DIR, BACKUP_EXT, DRY_RUN

${BOLD}EXAMPLES${RESET}
    # Apply all patches in ./patches/ to the current directory
    $0

    # Apply a single patch, dry run, no backups
    $0 --dry-run --backup "" fix-issue-42.patch

    # Apply all patches to a specific source tree
    $0 --target /src/myproject --patches /work/patches

USAGE
}

# ---------------------------------------------------------------------------
# Parse CLI args
# ---------------------------------------------------------------------------
EXPLICIT_PATCH=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        -t|--target)   TARGET_DIR="$2";  shift 2 ;;
        -p|--patches)  PATCH_DIR="$2";   shift 2 ;;
        -b|--backup)   BACKUP_EXT="$2";  shift 2 ;;
        -n|--dry-run)  DRY_RUN=1;        shift   ;;
        -h|--help)     usage; exit 0               ;;
        -*)            die "Unknown option: $1"     ;;
        *)             EXPLICIT_PATCH="$1"; shift  ;;
    esac
done

# ---------------------------------------------------------------------------
# Validate environment
# ---------------------------------------------------------------------------
command -v python3 &>/dev/null || die "python3 is required but not found in PATH"

[[ -d "$TARGET_DIR" ]] || die "Target directory not found: $TARGET_DIR"

# ---------------------------------------------------------------------------
# Build list of patches to apply
# ---------------------------------------------------------------------------
declare -a PATCHES=()

if [[ -n "$EXPLICIT_PATCH" ]]; then
    [[ -f "$EXPLICIT_PATCH" ]] || die "Patch file not found: $EXPLICIT_PATCH"
    PATCHES=("$EXPLICIT_PATCH")
else
    [[ -d "$PATCH_DIR" ]] || die "Patch directory not found: $PATCH_DIR"
    while IFS= read -r -d '' f; do
        PATCHES+=("$f")
    done < <(find "$PATCH_DIR" -maxdepth 1 -name '*.patch' -print0 | sort -z)
    [[ ${#PATCHES[@]} -gt 0 ]] || die "No .patch files found in: $PATCH_DIR"
fi

# ---------------------------------------------------------------------------
# Summary banner
# ---------------------------------------------------------------------------
echo ""
echo -e "${BOLD}════════════════════════════════════════════${RESET}"
echo -e "${BOLD} Source Patch Runner${RESET}"
echo -e "${BOLD}════════════════════════════════════════════${RESET}"
info "Target  : $TARGET_DIR"
info "Patches : ${#PATCHES[@]} file(s)"
info "Backups : ${BACKUP_EXT:-"(disabled)"}"
[[ "$DRY_RUN" == "1" ]] && warn "DRY RUN mode — no files will be modified"
echo ""

# ---------------------------------------------------------------------------
# Apply each patch
# ---------------------------------------------------------------------------
PASS=0
FAIL=0

for patch in "${PATCHES[@]}"; do
    echo -e "${BOLD}── $(basename "$patch") ──${RESET}"

    py_args=("$patch" "$TARGET_DIR")
    [[ "$DRY_RUN"    == "1"  ]] && py_args+=("--dry-run")
    [[ -n "$BACKUP_EXT"      ]] && py_args+=("--backup" "$BACKUP_EXT")

    if python3 "$PATCHER_PY" "${py_args[@]}"; then
        ok "Patch applied: $(basename "$patch")"
        (( PASS++ )) || true
    else
        error "Patch FAILED: $(basename "$patch")"
        (( FAIL++ )) || true
    fi
    echo ""
done

# ---------------------------------------------------------------------------
# Final summary
# ---------------------------------------------------------------------------
echo -e "${BOLD}════════════════════════════════════════════${RESET}"
if [[ $FAIL -eq 0 ]]; then
    ok "${BOLD}All $PASS patch(es) applied successfully.${RESET}"
else
    warn "Passed: $PASS  |  Failed: $FAIL"
    echo -e "${RED}Some patches failed. Check output above.${RESET}"
    exit 1
fi
