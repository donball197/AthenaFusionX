import os, re

# 1. Inject Dependencies
cargo_path = "bins/cli/Cargo.toml"
if os.path.exists(cargo_path):
    with open(cargo_path, "r") as f: data = f.read()
    if "rustyline" not in data:
        with open(cargo_path, "a") as f:
            f.write('\nrustyline = "14.0"\nindicatif = "0.17"\n')

# 2. Inject UX Logic
main_path = "bins/cli/src/main.rs"
if os.path.exists(main_path):
    with open(main_path, "r") as f: data = f.read()

    # Suture A: The Rustyline REPL
    data = re.sub(
        r"let stdin = io::stdin\(\);\s*// ⚡ The Interactive REPL\s*loop \{\s*print!\(.*?\);\s*io::stdout\(\)\.flush\(\)\?;\s*let mut input = String::new\(\);\s*stdin\.read_line\(&mut input\)\?;\s*let command = input\.trim\(\);",
        """let mut rl = rustyline::DefaultEditor::new().unwrap();
    // ⚡ The Interactive REPL
    loop {
        let readline = rl.readline("\\x1b[1;32mnexus>\\x1b[0m ");
        let command_str = match readline {
            Ok(line) => line,
            Err(_) => break,
        };
        let command = command_str.trim();
        if !command.is_empty() {
            let _ = rl.add_history_entry(command);
        }""",
        data,
        flags=re.DOTALL
    )

    # Suture B: The Indicatif API Spinner (Start)
    data = re.sub(
        r"println!\(\"\\x1b\[36m>> Engaging Neural Routing\.\.\.\\x1b\[0m\"\);\s*// ⚡ Forward directly to the Decoupled Task Manager",
        """let spinner = indicatif::ProgressBar::new_spinner();
                spinner.set_style(indicatif::ProgressStyle::default_spinner().template("{spinner:.magenta} \\x1b[36m>> Engaging Neural Routing...\\x1b[0m").unwrap());
                spinner.enable_steady_tick(std::time::Duration::from_millis(80));
                // ⚡ Forward directly to the Decoupled Task Manager""",
        data,
        flags=re.DOTALL
    )

    # Suture C: The Indicatif API Spinner (Finish)
    data = re.sub(
        r"\)\.await;\s*println!\(\"\\n\\x1b\[1;32m\[MISSION COMPLETE\]\\x1b\[0m \{\}\", response\);",
        """).await;
                spinner.finish_and_clear();
                println!("\\n\\x1b[1;32m[MISSION COMPLETE]\\x1b[0m {}", response);""",
        data,
        flags=re.DOTALL
    )

    with open(main_path, "w") as f: f.write(data)
    print("\x1b[32m[OK]\x1b[0m CLI Visual Upgrades Forged.")
else:
    print("\x1b[31m[FAULT]\x1b[0m bins/cli/src/main.rs not found.")
