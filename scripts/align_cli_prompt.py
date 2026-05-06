from pathlib import Path

f = Path("src/cli.rs")

code = f.read_text()

old = r'''MANDATORY OUTPUT:

<THOUGHT>
brief reasoning
</THOUGHT>

<ACTION>
[
 {json action},
 {json action}
]
</ACTION>'''

new = r'''MANDATORY OUTPUT:

Return ONLY a valid JSON array.

Do NOT output:
- XML tags
- Markdown
- Code fences
- Explanations

Each object must contain:
{
  "type": "...",
  "reasoning": "...",
  ...action fields...
}'''

if old in code:
    code = code.replace(old, new)
    f.write_text(code)
    print(">> [PATCH SUCCESS] cli.rs aligned to JSON schema.")
else:
    print(">> [WARN] old prompt block not found.")
