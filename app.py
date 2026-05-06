from flask import Flask, render_template
import csv
import os

app = Flask(__name__)

CSV_FILE = 'prices.csv'

def get_latest_price():
    if not os.path.exists(CSV_FILE):
        return "N/A", "File not found"
    
    try:
        with open(CSV_FILE, mode='r') as file:
            reader = list(csv.DictReader(file))
            if reader:
                latest = reader[-1]
                return latest['price'], latest['timestamp']
            return "N/A", "No data in CSV"
    except Exception as e:
        return "Error", str(e)

@app.route('/')
def index():
    price, timestamp = get_latest_price()
    return render_template('index.html', price=price, timestamp=timestamp)

if __name__ == '__main__':
    # Running on 0.0.0.0 to allow external access if needed
    app.run(host='0.0.0.0', port=5000)