import requests
import time

# Hard-coded confirmed IP: 192.168.43.15
TABLET_IP = "192.168.43.15" 
URL = f"http://{TABLET_IP}:3000/api/audit"

def run_audit():
    print(f">> [NEXUS] Targeted Pulse: {TABLET_IP}")
    print(f">> [SINGULARITY] Initiating Swarm Audit (68 Agents) on 5GB Source...")
    
    payload = {
        "path": "/home/donball197/nexus_singularity/src",
        "agent_swarm": 68,
        "depth": "deep"
    }
    
    try:
        start = time.time()
        res = requests.post(URL, json=payload, timeout=60)
        duration = time.time() - start
        
        if res.status_code == 200:
            print(f">> [SUCCESS] Audit Complete in {duration:.2f}s")
            print(">> [REPORT] Logic scan finalized. Check tablet terminal for agent logs.")
        else:
            print(f">> [FAIL] Tablet returned status: {res.status_code}")
    except Exception as e:
        print(f">> [ERROR] Bridge still blocked. Is the tablet listening? {e}")

if __name__ == "__main__":
    run_audit()
