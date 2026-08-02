import logging
from fastapi import FastAPI
from pydantic import BaseModel
from fastembed import TextEmbedding

logging.getLogger("fastembed").setLevel(logging.ERROR)

print("\033[36m>> [MINION] Booting ONNX Runtime for all-MiniLM-L6-v2...\033[0m")
app = FastAPI()

model = TextEmbedding("sentence-transformers/all-MiniLM-L6-v2")
print("\033[32m>> [MINION] Model loaded. 384-Dimension space established.\033[0m")

class EmbedRequest(BaseModel):
    text: str

@app.post("/embed")
def embed(req: EmbedRequest):
    vectors = list(model.embed([req.text]))
    return {"embedding": vectors[0].tolist()}
