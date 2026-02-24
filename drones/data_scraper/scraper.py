from flask import Flask, jsonify
import csv
import urllib.request
import json
import os

app = Flask(__name__)
DATA_FILE = 'bitcoin_history.csv'

# Ensure CSV exists with headers
if not os.path.exists(DATA_FILE):
    with open(DATA_FILE, mode='w', newline='') as file:
        writer = csv.writer(file)
        writer.writerow(['Currency', 'Price_USD'])

@app.route('/scrape', methods=['GET'])
def scrape():
    try:
        # Fetch mock or real data (CoinGecko public API)
        url = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd"
        req = urllib.request.Request(url, headers={'User-Agent': 'Nexus-Drone/1.0'})
        response = urllib.request.urlopen(req).read()
        data = json.loads(response)
        price = data['bitcoin']['usd']
        
        # Write to CSV (Mission 1)
        with open(DATA_FILE, mode='a', newline='') as file:
            writer = csv.writer(file)
            writer.writerow(['bitcoin', price])
            
        return jsonify({"status": "success", "bitcoin_usd": price, "action": "CSV updated"})
    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500

@app.route('/health', methods=['GET'])
def health():
    return jsonify({"status": "online", "drone": "data_scraper"})

if __name__ == '__main__':
    # Run Flask server (Mission 2)
    app.run(host='0.0.0.0', port=5000)
