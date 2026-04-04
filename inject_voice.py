import os

with open('ui/src/routes/+page.svelte', 'r') as f:
    code = f.read()

# 1. Inject Import
import_target = "import LazarusTimeline from '$lib/components/LazarusTimeline.svelte';"
import_injection = import_target + "\n    import VoiceUplink from '$lib/components/VoiceUplink.svelte';"

# 2. Inject Handler Logic
handler_target = "onMount(() => {"
handler_injection = """
    function handleVoiceCommand(type, payload = "") {
        if (type === "execute") executeAction();
        else if (type === "prompt") { userPrompt = payload; executeAction(); }
        else if (type === "ghost_type" && editorRef && currentCode) {
            isGhostTyping.set(true);
            editorRef.typeIntoEditor(currentCode + "\\n// Ghost typed via neural uplink", vitals.resonance);
        }
        else if (type === "show_timeline") showTimeline = true;
        else if (type === "clear") neuralResponse = ">> CLEARED BY VOICE";
    }

    onMount(() => {"""

# 3. Inject Floating UI Component
ui_target = "<MergeGate"
ui_injection = """
<div class="fixed bottom-6 right-6 z-[400] w-64">
    <VoiceUplink onCommand={handleVoiceCommand} resonance={vitals.resonance} />
</div>

<MergeGate"""

if "VoiceUplink" not in code:
    code = code.replace(import_target, import_injection)
    code = code.replace(handler_target, handler_injection)
    code = code.replace(ui_target, ui_injection)
    with open('ui/src/routes/+page.svelte', 'w') as f:
        f.write(code)
    print(">> [SUCCESS] Voice Uplink injected surgically without breaking grid or PatchFile logic.")
else:
    print(">> [SKIP] Voice Uplink already exists in file.")
