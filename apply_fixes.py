import re
import os

def patch_epistemic_engine():
    path = "crates/runtime_core/src/epistemic_engine.rs"
    print(f"Patching {path}...")
    
    with open(path, "r") as f:
        content = f.read()

    # The exact regex pattern to find the Action Strategist prompt block
    pattern = r'(let prompt = format!\("You are the Action Strategist\.\\nORIGINAL DIRECTIVE.*?mission, strategy, world_context, evidence_so_far\);)'

    new_prompt = '''let prompt = format!("You are the Action Strategist.\\nORIGINAL DIRECTIVE: {}\\nSTRATEGY: {}\\nWORLD CONTEXT: {}\\nEVIDENCE GATHERED SO FAR:\\n{}\\nPropose the exact next step to fulfill the strategy.\\nCRITICAL RULE 0: Do NOT propose directory-listing or file-discovery commands (ls -R, find, cat on an unconfirmed path) if the relevant file tree or path is already visible in WORLD CONTEXT above.\\nCRITICAL RULE 1: If the EVIDENCE GATHERED SO FAR contains an exact file path outputted by symbol_graph or a previous command, you MUST use that exact path.\\nCRITICAL RULE 2: If the EVIDENCE GATHERED SO FAR contains a Critic rejection or veto (marked [Contradicts]) that suggests a specific tool, command, or action to try next, your proposal MUST adopt that suggested action. DO NOT RE-PROPOSE AN ACTION THAT WAS JUST REJECTED.\\nCRITICAL RULE 3: If the task requires generating a report, analysis, or diagram, you MUST propose writing the final synthesized output to a file using 'write_file <path> <content>' BEFORE you halt. If the EVIDENCE GATHERED SO FAR proves your current strategy is fully complete with a [WRITE_OK] or [CMD_OK] output satisfying the directive, your IMMEDIATE next proposal MUST be 'halt'.\\nCRITICAL RULE 4: TOOLCHAIN TRUST & EXPLORATORY GRACE\\nIf the Strategist proposes executing a workspace tool (e.g., benchmark_runner, mutation_ranker, execution_auditor, impact_analyzer) via target/release/ or cargo run, you MUST ASSUME the tool exists and is authorized. Do NOT veto execution on the basis of missing prerequisite files (like trace logs, json reports, or candidates) if the tool is actively being invoked to analyze a target. Allow exploratory tool execution. Do not repeat successful commands.\\nCRITICAL RULE 5: When invoking workspace binaries (impact_analyzer, execution_auditor, mutation_ranker, symbol_graph), ALWAYS use 'cargo run --bin <bin_name> -- <args>' or the absolute debug path '/home/donball197/AthenaFusionX/target/debug/<bin_name>'. NEVER execute 'apps/<bin_name>' directly because 'apps/' is a source directory, not an executable binary.\\nCRITICAL RULE 6: SCRIPT ENVIRONMENT: Any proposed inline scripts (Python/Bash) MUST rely strictly on standard system libraries (e.g., os, sys, json in Python). Do NOT assume third-party packages like 'toml' or 'requests' are installed.\\nOUTPUT ONLY A SINGLE LINE CONTAINING THE EXACT COMMAND OR ACTION. DO NOT INCLUDE EXPLANATIONS, REASONING, OR PREAMBLE.", mission, strategy, world_context, evidence_so_far);'''

    if re.search(pattern, content, flags=re.DOTALL):
        content = re.sub(pattern, new_prompt, content, flags=re.DOTALL)
        with open(path, "w") as f:
            f.write(content)
        print("  -> epistemic_engine.rs successfully patched (Bug #2 & #3 fixed).")
    else:
        print("  -> ERROR: Could not find the Strategist prompt block.")

def patch_world_model():
    path = "crates/runtime_core/src/world_model.rs"
    print(f"Patching {path}...")
    
    with open(path, "r") as f:
        content = f.read()

    # The exact regex pattern to find the truncated file tree push
    pattern = r'(\s*if !files\.is_empty\(\) \{\s*chunks\.push\("\[REPOSITORY FILE TREE \(Depth 4\)\]"\.to_string\(\)\);\s*chunks\.push\(files\.chars\(\)\.take\(6000\)\.collect\(\)\);\s*\})'

    new_block = '''
        // [FIX] Disabled truncated raw file tree to prevent 'apps/' vs 'crates/' alphabetical hallucination.
        // if !files.is_empty() {
        //     chunks.push("[REPOSITORY FILE TREE (Depth 4)]".to_string());
        //     chunks.push(files.chars().take(6000).collect());
        // }'''

    if re.search(pattern, content):
        content = re.sub(pattern, new_block, content)
        with open(path, "w") as f:
            f.write(content)
        print("  -> world_model.rs successfully patched (Bug #1 fixed).")
    else:
        print("  -> ERROR: Could not find the file tree truncation block.")

if __name__ == "__main__":
    print("Executing AthenaFusionX Cortex Patches...")
    patch_epistemic_engine()
    patch_world_model()
    print("Done.")
