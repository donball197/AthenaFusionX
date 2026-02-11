import requests
import json
import time

# Master Node URL
URL = "http://localhost:3000/api/process"

def run_test():
    payload = {
        "prompt": "INITIATE 68-AGENT NEURAL SWARM: Analyze network entropy and optimize distributed RAM synchronization across cluster.",
        "agent_count": 68
    }

    print(">> [NEXUS] Initiating Stress Test...")
    print(">> [NEXUS] Payload: 68 Native Agents")
    
    try:
        start_time = time.time()
        response = requests.post(URL, json=payload, timeout=15)
        duration = time.time() - start_time
        
        if response.status_code == 200:
            data = response.json()
            print(f"\n>> [SUCCESS] Response received in {duration:.2f}s")
            print(f">> [NODE ID] {data['node_id']}")
            print(f">> [RESULT] {data['result']}")
        else:
            print(f">> [FAIL] Node returned status: {response.status_code}")
            
    except Exception as e:
        print(f">> [CRITICAL] Cluster communication failed: {e}")

if __name__ == "__main__":
    run_test()
