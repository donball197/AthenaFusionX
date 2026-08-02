#!/usr/bin/env python3
import sys
import os
import argparse
import subprocess

def apply_patch(diff_content):
    """Applies a raw text diff safely by dumping to a temp file and using git apply or standard patch."""
    patch_file = "/tmp/cortex_mutation.patch"
    with open(patch_file, "w") as f:
        f.write(diff_content)

    print(f">> [PATCH ENGINE] Attempting to apply diff from {patch_file}...")

    # 1. Try standard 'patch' first (highly reliable for raw hunks)
    try:
        result = subprocess.run(
            ["patch", "-p1", "-i", patch_file],
            capture_output=True, text=True, check=True
        )
        print(">> [SUCCESS] Patch applied via GNU patch.")
        print(result.stdout)
        return True
    except subprocess.CalledProcessError as e:
        print(">> [WARNING] GNU patch failed. Retrying with git apply...")
        print("GNU patch stderr:", e.stderr)

    # 2. Fallback to 'git apply' if it's a formatted git diff
    try:
        result = subprocess.run(
            ["git", "apply", "--ignore-space-change", "--ignore-whitespace", patch_file],
            capture_output=True, text=True, check=True
        )
        print(">> [SUCCESS] Patch applied via git apply.")
        print(result.stdout)
        return True
    except subprocess.CalledProcessError as e:
        print(">> [FATAL] Git apply also failed. Mutation rejected.")
        print("Git apply stderr:", e.stderr)
        return False

if __name__ == "__main__":
    # If the AI pipes the diff via stdin (echo '...' | python3 patch.py)
    if not sys.stdin.isatty():
        diff_content = sys.stdin.read()
        if not diff_content.strip():
            print(">> [ERROR] Patch engine received empty stdin.")
            sys.exit(1)
        
        success = apply_patch(diff_content)
        sys.exit(0 if success else 1)
    
    # If the AI passes a file directly
    parser = argparse.ArgumentParser(description="AthenaFusionX Unified Patch Engine")
    parser.add_argument("file", help="Path to the .patch or .diff file")
    args = parser.parse_args()

    if not os.path.exists(args.file):
        print(f">> [ERROR] Patch file not found: {args.file}")
        sys.exit(1)

    with open(args.file, "r") as f:
        diff_content = f.read()
    
    success = apply_patch(diff_content)
    sys.exit(0 if success else 1)
