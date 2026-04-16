import re
with open('src/server.rs', 'r') as f:
    content = f.read()

# Replace the first send_task call (for "task <directive>")
content = re.sub(
    r'task_manager::send_task\((.*?)\)\s*\.await;\s*println!\("\\x1b\[32m>> \[CLI\] Task complete\.\\x1b\[0m"\);',
    r'''if let Err(e) = task_manager::send_task(\1).await {
                    eprintln!("\x1b[31m!! [CORE_FAULT] Ghost API Disconnected: {}\x1b[0m", e);
                } else {
                    println!("\x1b[32m>> [CLI] Task complete.\x1b[0m");
                }''',
    content,
    flags=re.DOTALL
)

# Replace the second send_task call (for raw directives)
content = re.sub(
    r'task_manager::send_task\((.*?)\)\s*\.await;',
    r'''if let Err(e) = task_manager::send_task(\1).await {
                    eprintln!("\x1b[31m!! [CORE_FAULT] Ghost API Disconnected: {}\x1b[0m", e);
                }''',
    content,
    flags=re.DOTALL
)

with open('src/server.rs', 'w') as f:
    f.write(content)
