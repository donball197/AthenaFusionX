import pathlib, sys

p = pathlib.Path("pass_b_graph_mechanical_subst.sh")
src = p.read_text(encoding="utf-8")

if 'done_probe="session.graph.decay(h_id, 0.20)"' in src:
    print("[skip] B5 wildcard patch already applied")
    sys.exit(0)


def one(src, old, new, tag):
    n = src.count(old)
    if n != 1:
        print("[abort] %s: anchor matched %dx; nothing written" % (tag, n))
        sys.exit(1)
    return src.replace(old, new)


HELPER = '''ANY = "<ANY>"


def _match_seq(i, seq):
    for k, want in enumerate(seq):
        if want == ANY:
            continue
        if lines[i + k].strip() != want:
            return False
    return True


'''

src = one(src, "def seq_hits(seq):", HELPER + "def seq_hits(seq):", "helper insert")

src = one(
    src,
    '        if all(lines[i + k].strip() == seq[k] for k in range(n)):',
    '        if _match_seq(i, seq):',
    "seq_hits wildcard")

src = one(
    src,
    "def replace_block(tag, old_seq, new_lines):",
    "def replace_block(tag, old_seq, new_lines, done_probe=None):",
    "replace_block signature")

src = one(
    src,
    '    if present([t for _, t in new_lines]):\n'
    '        skip(tag + " (already applied)")\n'
    '        return',
    '    joined = "\\n".join(lines)\n'
    '    if done_probe and done_probe in joined:\n'
    '        skip(tag + " (already applied)")\n'
    '        return\n'
    '    if "<OLD:" not in "".join(t for _, t in new_lines) and present([t for _, t in new_lines]):\n'
    '        skip(tag + " (already applied)")\n'
    '        return',
    "replace_block skip logic")

src = one(
    src,
    '    out = []\n'
    '    for delta, text in new_lines:\n'
    '        out.append(" " * (base + 4 * delta) + text)',
    '    matched = [lines[i + k].strip() for k in range(len(old_seq))]\n'
    '    out = []\n'
    '    for delta, text in new_lines:\n'
    '        while "<OLD:" in text:\n'
    '            a = text.index("<OLD:")\n'
    '            b = text.index(">", a)\n'
    '            idx = int(text[a + 5:b])\n'
    '            text = text[:a] + matched[idx] + text[b + 1:]\n'
    '        out.append(" " * (base + 4 * delta) + text)',
    "replace_block OLD refs")

MARK_A = "# ------------------------------------------------- B5"
MARK_B = "# ------------------------------------------------- B6"

i = src.find(MARK_A)
j = src.find(MARK_B)
if i < 0 or j < 0 or j <= i:
    print("[abort] B5/B6 section markers not found; nothing written")
    sys.exit(1)

NEW_B5 = '''# ------------------------------------------------- B5
print()
print("=== [B5] decay (parse-fault, 1894) - same arithmetic ===")
# The log call carries an ANSI escape string literal. The pattern never
# matches its content: old-block line 5 is <ANY>, and the replacement
# re-emits it verbatim via <OLD:4>. No escape sequence appears anywhere
# in this script.
replace_block(
    "B5 parse-fault decay via graph.decay",
    [
        "if let Some(h_id) = session.best_hypothesis_id {",
        "if let Some(node) = session.graph.nodes.get_mut(&h_id) {",
        "node.belief_score = (node.belief_score - 0.20).max(0.0);",
        "self.log(format!(",
        "<ANY>",
        "node.belief_score",
        "));",
        "}",
        "}",
    ],
    [
        (0, "if let Some(h_id) = session.best_hypothesis_id {"),
        (1, "if let Some(new_belief) = session.graph.decay(h_id, 0.20) {"),
        (2, "self.log(format!("),
        (3, "<OLD:4>"),
        (3, "new_belief"),
        (2, "));"),
        (1, "}"),
        (0, "}"),
    ],
    done_probe="session.graph.decay(h_id, 0.20)")


'''

src = src[:i] + NEW_B5 + src[j:]

p.write_text(src, encoding="utf-8")
print("[ok] engine upgraded: <ANY> wildcard + <OLD:k> line splicing")
print("[ok] B5 section rewritten; escape sequence never transported")
