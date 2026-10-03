import pathlib, sys

p = pathlib.Path("pass_a_graph_api_additive.sh")
src = p.read_text(encoding="utf-8")

FIXED = 'parts = [x.strip() for x in params.split(",") if ":" in x]'

if FIXED in src:
    print("[skip] fix already present")
    sys.exit(0)

cands = [
    'parts = [x.strip() for x in params.split(",")]',
    'parts = [x.strip() for x in params.split(",") ]',
]
hits = [c for c in cands if c in src]

if len(hits) != 1:
    print("[abort] expected exactly one unfixed variant, found %d" % len(hits))
    print("[diag] candidates present: %r" % hits)
    sys.exit(1)

old = hits[0]
src = src.replace(old, FIXED)

# sanity: the surrounding lines still exist exactly once
for probe in ("def fn_signature", "p4_name, p4_type = parts[3].split", "FORBIDDEN"):
    if probe not in src:
        print("[abort] script looks damaged near %r; refusing to write" % probe)
        sys.exit(1)

p.write_text(src, encoding="utf-8")
print("[ok] A3 param filter now excludes the self receiver")
print("[ok] script integrity probes passed")
