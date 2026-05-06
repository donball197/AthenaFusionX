import os

path = "ui/src/routes/workspace/+page.svelte"
if os.path.exists(path):
    with open(path, "r") as f:
        code = f.read()

    # Add the import
    import_line = "import EditorPanel from '$lib/components/panels/EditorPanel.svelte';"
    new_import = import_line + "\n    import SecurityPanel from '$lib/components/panels/SecurityPanel.svelte';"
    
    # Add the component above the grids
    target_spot = '<div class="grid grid-cols-1 md:grid-cols-3 gap-6">'
    new_spot = '<div class="mb-6"><SecurityPanel /></div>\n        ' + target_spot

    if import_line in code and "SecurityPanel" not in code:
        code = code.replace(import_line, new_import)
        code = code.replace(target_spot, new_spot)
        with open(path, "w") as f:
            f.write(code)
        print("\x1b[32m>> [SUTURE] Security Pulse mounted in UI.\x1b[0m")
    else:
        print("\x1b[33m>> [SKIP] Component already mounted or path not found.\x1b[0m")
