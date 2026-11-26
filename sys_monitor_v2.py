import json, os
def get_metrics():
    return {'load_avg': os.getloadavg()}
print(json.dumps(get_metrics()))
