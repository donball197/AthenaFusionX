import requests
import time

URL = "http://192.168.43.15:3000/api/process"

def ignite_swarm():
    print(f">> [NEXUS] Targeted Pulse: 192.168.43.15")
    print(">> [SINGULARITY] Waking up 68 Agents...")
    
    success = 0
    for i in range(1, 51):
        try:
            res = requests.post(URL, json={"prompt": "wakeup", "agent_count": 68}, timeout=2)
            if res.status_code == 200:
                success += 1
            if i % 10 == 0:
                print(f">> [PULSE] {i}/50 processed by Worker.")
        except:
            continue
            
    print(f"\n>> [RESULT] {success}/50 Successful Handshakes.")
    if success > 0:
        print(">> [STATUS] THE TABLET IS AWAKE.")
    else:
        print(">> [ERROR] Bridge still blocked. Check Tablet Engine.")

if __name__ == "__main__":
    ignite_swarm()
