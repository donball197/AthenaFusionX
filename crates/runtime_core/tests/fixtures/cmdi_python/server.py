import ipaddress
import subprocess
from flask import Flask, request

app = Flask(__name__)

@app.route('/ping')
def ping_ip():
    ip = request.args.get('ip', '')
    try:
        # Validate that the input is a valid IP address or domain/hostname
        # For strict IP validation: ipaddress.ip_address(ip)
        # Or validate using regex/ipaddress
        parsed_ip = str(ipaddress.ip_address(ip))
    except ValueError:
        return "Invalid IP address", 400
    
    try:
        result = subprocess.run(
            ["ping", "-c", "1", parsed_ip],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=5
        ).stdout
    except Exception as e:
        result = str(e)
    return f"<pre>{result}</pre>"

@app.route('/backup')
def backup_files():
    folder = request.args.get('folder', '')
    os.system(f"tar -czvf backup.tar.gz /var/backups/{folder}")
    return "Done"

if __name__ == '__main__':
    app.run()
