use chrono::Local;

fn main() {
    let now = Local::now();
    println!("Demo 2 - Current time: {}", now.format("%Y-%m-%d %H:%M:%S"));
}