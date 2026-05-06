import sqlite3
import base64

WASM_B64 = "AGFzbQEAAAABCAJmAX8Bf2ADf39/AX8DBQIBAAcRAQ50aGVybWFsX2ZpbHRlcgAACgkBBf0BAEEBv79/C0EKAA=="

db = sqlite3.connect(".nexus_wasm_registry.db")
cur = db.cursor()
cur.execute("""
    INSERT OR REPLACE INTO wasm_cells (name, wasm_b64, source_code)
    VALUES (?, ?, ?)
""", ("thermal_filter", WASM_B64, "pub extern \"C\" fn thermal_filter(temp: f32) -> f32 { if temp > 60.0 { 1.0 } else { 0.0 } }"))
db.commit()
db.close()
print(">> [REGISTRY] thermal_filter injected manually for RAM bypass.")
