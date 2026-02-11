import requests
import time

# Target: Lenovo Duet 2 (Crostini IP)
# REMINDER: Ensure Port 3000 is Forwarded in ChromeOS Settings
URL = "http://100.115.92.197:3000/api/process"

def run_stress():
    print(">> [NEXUS] Initiating 50-Pulse Stress Test on Lenovo Duet 2...")
    success = 0
    for i in range(1, 51):
        try:
            # Bombarding with 68-agent compute requests
            res = requests.post(URL, json={"prompt": "leak_check", "agent_count": 68}, timeout=1)
            if res.status_code == 200:
                success += 1
            if i % 10 == 0:
                print(f">> [PULSE] {i}/50 sent.")
        except:
            continue
    print(f"\n>> [COMPLETE] {success}/50 reached the target.")
    print(">> Check the Tablet's RSS now. It should settle back near 16736.")

if __name__ == "__main__":
    run_stress()
