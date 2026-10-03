import pathlib, sys

p = pathlib.Path("pass_a_graph_api_additive.sh")
src = p.read_text(encoding="utf-8")

FILTER = 'params.split(",") if ":" in x]'
UNFILTERED = 'params.split(",")]'

has_filter = FILTER in src
has_unfiltered = UNFILTERED in src

print("[diag] filter line present:     %s" % has_filter)
print("[diag] unfiltered line present: %s" % has_unfiltered)

if has_filter == has_unfiltered:
    print("[abort] neither or both parts= variants present; ambiguous, nothing written")
    sys.exit(1)

out = src

# 1) len/index: any of the coherent forms -> 4 / parts[3]
for old in (
    'if len(parts) != 5 or ":" not in parts[4]:',
    'if len(parts) != 5 or ":" not in parts[3]:',
    'if len(parts) != 4 or ":" not in parts[4]:',
):
    out = out.replace(old, 'if len(parts) != 4 or ":" not in parts[3]:')

for old in ('parts[4].split(":", 1)', 'parts[3].split(":", 1)'):
    out = out.replace(old, 'parts[3].split(":", 1)')

# 2) if the unfiltered parts= line is what is on disk, add the filter
if has_unfiltered:
    out = out.replace('parts = [x.strip() for x in params.split(",")]',
                      'parts = [x.strip() for x in params.split(",") if ":" in x]')

# 3) coherence gate: no residual parts[4], exactly one len check
residual = [(i + 1, l) for i, l in enumerate(out.split("\n")) if "parts[4]" in l]
if residual:
    print("[abort] residual parts[4] references; refusing to write")
    for n, l in residual:
        print("       %5d: %s" % (n, l))
    sys.exit(1)

if out.count('if len(parts) != 4 or ":" not in parts[3]:') != 1:
    print("[abort] len check not uniquely normalized; refusing to write")
    sys.exit(1)

# 4) script integrity probes
for probe in ("def fn_signature", "FORBIDDEN", "restore_and_die",
              "cost_name, cost_type = parts[3].split", "pub fn observe_contradiction"):
    if probe not in out:
        print("[abort] script damaged near %r; refusing to write" % probe)
        sys.exit(1)

if out == src:
    print("[skip] already coherent")
    sys.exit(0)

p.write_text(out, encoding="utf-8")
print("[ok] A3 normalized: filter-based, len(parts) != 4, parts[3] is cost")
