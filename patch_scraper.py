import os

path = "src/scraper_drone.rs"
print("[*] Synchronizing Scraper Drone to Memory Palace v4.5...")

with open(path, "r") as f:
    code = f.read()

# Change method name and add the 0.0 credits argument
code = code.replace(
    '.store_memory("ALPHA-SCOUT", &title, &result_summary)',
    '.remember("ALPHA-SCOUT", &title, &result_summary, 0.0)'
)

with open(path, "w") as f:
    f.write(code)

print("[+] Scraper Drone aligned.")
