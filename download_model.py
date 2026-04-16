import kagglehub
import os
import shutil

# Download the specific Gemma 2 2B INT4 model
path = kagglehub.model_download("google/gemma-2/tfLite/gemma2-2b-it-cpu-int4")

print(f"Model downloaded to: {path}")

# Move it to your Project's vault
os.makedirs("vault", exist_ok=True)
for file in os.listdir(path):
    if file.endswith(".tflite") or file.endswith(".task"):
        shutil.copy(os.path.join(path, file), "vault/gemma.tflite")
        print(f"Successfully moved to vault/gemma.tflite")
