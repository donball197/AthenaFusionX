import pathlib, sys

p = pathlib.Path("pass_c_f4_unclamp.sh")
src = p.read_text(encoding="utf-8")

A_OLD = "if any(l.strip() == new for l in lines):"
A_NEW = "if not any(l.strip() == old for l in lines) and any(l.strip() == new for l in lines):"

B_OLD = "if seq_hits(new_seq):"
B_NEW = "if not seq_hits(old_seq) and seq_hits(new_seq):"

if A_NEW in src and B_NEW in src:
    print("[skip] skip-logic patch already applied")
    sys.exit(0)

for tag, old, new in (("replace_line", A_OLD, A_NEW),
                      ("replace_block", B_OLD, B_NEW)):
    n = src.count(old)
    if n != 1:
        print("[abort] %s: anchor matched %dx; nothing written" % (tag, n))
        sys.exit(1)
    src = src.replace(old, new)

p.write_text(src, encoding="utf-8")
print("[ok] replace_line: apply whenever the old line is still present")
print("[ok] replace_block: apply whenever the old block is still present")
