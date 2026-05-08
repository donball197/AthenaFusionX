#!/bin/bash
# ⚡ NEXUS DNA HARVESTER v1.0
# Aggregates the entire workspace into a single context block for the 1M API window.

OUTPUT_FILE="workspace_dna.txt"
echo ">> Harvesting Substrate DNA..."

echo "=== PROJECT STRUCTURE ===" > $OUTPUT_FILE
tree -I 'target|node_modules|.git|.zig-cache' >> $OUTPUT_FILE

echo -e "\n=== SOURCE CODE DNA ===" >> $OUTPUT_FILE

# Find all relevant files and wrap them in XML-style tags for the LLM
find crates bins -type f \( -name "*.rs" -o -name "*.zig" -o -name "*.toml" \) \
  -not -path "*/target/*" \
  -not -path "*/.zig-cache/*" | while read -r file; do
    echo "<FILE path=\"$file\">" >> $OUTPUT_FILE
    cat "$file" >> $OUTPUT_FILE
    echo -e "</FILE>\n" >> $OUTPUT_FILE
done

SIZE=$(du -h $OUTPUT_FILE | cut -f1)
echo ">> [SUCCESS] DNA Harvested ($SIZE) into $OUTPUT_FILE"
echo ">> You can now pipe this directly into your API System Prompt."
