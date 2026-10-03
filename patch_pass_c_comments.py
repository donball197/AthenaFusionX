import pathlib, sys

p = pathlib.Path("pass_c_f4_unclamp.sh")
src = p.read_text(encoding="utf-8")

if "crosses the selection floor" in src:
    print("[skip] comment fix already applied")
    sys.exit(0)


def one(src, old, new, tag):
    n = src.count(old)
    if n != 1:
        print("[abort] %s: anchor matched %dx; nothing written" % (tag, n))
        sys.exit(1)
    return src.replace(old, new)


# The fix rides along inside C3a's NEW block (applied to the test body
# on the same edit), keeping one pass = one coherent change set.
src = one(
    src,
    '        "CorticalLoop::decay_hypothesis_belief(&mut session, h_id, 0.20);",\n'
    '        "assert_eq!(session.graph.belief_of(h_id), Some((0.30_f32 - 0.20_f32).max(0.0)));",',
    '        "// Pre-decay 0.30, delta 0.20 -> 0.10: the belief CROSSES",\n'
    '        "// SELECT_FLOOR and becomes unselectable (F4: no clamp).",\n'
    '        "CorticalLoop::decay_hypothesis_belief(&mut session, h_id, 0.20);",\n'
    '        "assert_eq!(session.graph.belief_of(h_id), Some((0.30_f32 - 0.20_f32).max(0.0)));",',
    "C3a new block: add corrected comments")

# The OLD block must grow to match, or the sequence match breaks.
src = one(
    src,
    '        "CorticalLoop::decay_hypothesis_belief_with_floor(&mut session, h_id, 0.20);",\n'
    '        "assert_eq!(session.graph.belief_of(h_id), Some(0.15));",',
    '        "// Pre-decay 0.51, delta 0.20 -> 0.31, above floor. No clamp.",\n'
    '        "// Pre-decay 0.30, delta 0.20 -> 0.10, below floor. Clamp to 0.15.",\n'
    '        "CorticalLoop::decay_hypothesis_belief_with_floor(&mut session, h_id, 0.20);",\n'
    '        "assert_eq!(session.graph.belief_of(h_id), Some(0.15));",',
    "C3a old block: capture the stale comments as well")

# Safety: the expectation count must stay exactly 1.
if src.count("(0.30_f32 - 0.20_f32).max(0.0)") != 1:
    print("[abort] expectation expression count drifted; nothing written")
    sys.exit(1)

p.write_text(src, encoding="utf-8")
print("[ok] C3a now rewrites the two stale test comments on the same edit")
print("[ok] old block captures them so the sequence match stays exact")
