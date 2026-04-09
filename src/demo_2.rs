use chrono::Local;

pub fn execute() {
    let now = Local::now();
    println!("Demo 2 - Current time: {}", now.format("%Y-%m-%d %H:%M:%S"));
}