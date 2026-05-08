import requests
import csv
import os

def scrape_bitcoin_price():
    url = "https://api.coindesk.com/v1/bpi/currentprice.json"
    try:
        response = requests.get(url)
        response.raise_for_status()
        data = response.json()
        price = data['bpi']['USD']['rate']
        
        with open('prices.csv', mode='w', newline='') as file:
            writer = csv.writer(file)
            writer.writerow(['Price'])
            writer.writerow([price])
        print(f"Successfully saved Bitcoin price: {price}")
    except Exception as e:
        print(f"Error scraping Bitcoin price: {e}")

if __name__ == "__main__":
    scrape_bitcoin_price()