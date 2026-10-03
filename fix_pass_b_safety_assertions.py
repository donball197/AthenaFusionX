import pathlib
import sys

p = pathlib.Path("pass_b_graph_mechanical_subst.sh")
src = p.read_text(encoding="utf-8")

old = '''    ("nodes.get_mut fully migrated", src.count("session.graph.nodes.get_mut") == 0),'''

new = '''    ("only deferred clamped-decay get_mut remains",
     src.count("session.graph.nodes.get_mut") == 1
     and src.count("if let Some(node) = session.graph.nodes.get_mut(&h_id) {") == 1
     and "fn decay_hypothesis_belief_with_floor(" in src),'''

if src.count(old) != 1:
    print("[abort] get_mut assertion anchor matched %dx; nothing written" % src.count(old))
    sys.exit(1)

src = src.replace(old, new)

old = '''    ("find_by_content x4", src.count("session.graph.find_by_content(") == 4),'''

new = '''    ("find_by_content x5", src.count("session.graph.find_by_content(") == 5),'''

if src.count(old) != 1:
    print("[abort] find_by_content assertion anchor matched %dx; nothing written" % src.count(old))
    sys.exit(1)

old = '''    ("nodes.get_mut residual", "session.graph.nodes.get_mut" not in written),'''

new = '''    ("only deferred clamped-decay get_mut remains",
     written.count("session.graph.nodes.get_mut") == 1
     and written.count("if let Some(node) = session.graph.nodes.get_mut(&h_id) {") == 1
     and "fn decay_hypothesis_belief_with_floor(" in written),'''

if src.count(old) != 1:
    print("[abort] post-write get_mut assertion anchor matched %dx; nothing written" % src.count(old))
    sys.exit(1)

p.write_text(src, encoding="utf-8")
print("[ok] Pass B safety assertions corrected")
print("[ok] exactly one get_mut is permitted: deferred clamped-decay helper")
print("[ok] find_by_content expected count corrected: 5")
