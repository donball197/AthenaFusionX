#!/usr/bin/env python3
import os
import json
import logging
from http.server import BaseHTTPRequestHandler, HTTPServer
import numpy as np

# Suppress heavy warnings
os.environ['TF_CPP_MIN_LOG_LEVEL'] = '3' 
logging.basicConfig(level=logging.INFO, format='%(asctime)s - [LITERT MINION] - %(message)s')

try:
    import tflite_runtime.interpreter as tflite
    from transformers import AutoTokenizer
    HAS_ML_DEPS = True
except ImportError:
    logging.warning("tflite_runtime or transformers not found. Running in MOCK mode for testing.")
    HAS_ML_DEPS = False

# We use the all-MiniLM-L6-v2 model (Generates 384-dimensional embeddings)
MODEL_NAME = "all-MiniLM-L6-v2"
TFLITE_PATH = f"../../vault/{MODEL_NAME}.tflite"

interpreter = None
tokenizer = None

if HAS_ML_DEPS and os.path.exists(TFLITE_PATH):
    logging.info(f"Loading LiteRT Model from {TFLITE_PATH}...")
    interpreter = tflite.Interpreter(model_path=TFLITE_PATH)
    interpreter.allocate_tensors()
    tokenizer = AutoTokenizer.from_pretrained(f"sentence-transformers/{MODEL_NAME}")
    logging.info("LiteRT Substrate Online. Memory usage < 50MB.")
else:
    logging.warning(f"Model not found at {TFLITE_PATH}. Proceeding with simulated fallback embeddings.")

class ReflexHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path == '/embed':
            content_length = int(self.headers.get('Content-Length', 0))
            post_data = self.rfile.read(content_length)
            
            try:
                data = json.loads(post_data.decode('utf-8'))
                text = data.get("text", "")
                
                embedding = self.generate_embedding(text)
                
                self.send_response(200)
                self.send_header('Content-type', 'application/json')
                self.end_headers()
                self.wfile.write(json.dumps({"embedding": embedding}).encode('utf-8'))
                
            except Exception as e:
                logging.error(f"Fault during embedding: {e}")
                self.send_response(500)
                self.end_headers()
        else:
            self.send_response(404)
            self.end_headers()

    def generate_embedding(self, text: str) -> list:
        # If we have the TFLite model loaded, use it natively
        if interpreter and tokenizer:
            inputs = tokenizer(text, padding=True, truncation=True, return_tensors="np")
            
            input_details = interpreter.get_input_details()
            output_details = interpreter.get_output_details()
            
            interpreter.set_tensor(input_details[0]['index'], inputs['input_ids'])
            if len(input_details) > 1:
                interpreter.set_tensor(input_details[1]['index'], inputs['attention_mask'])
                
            interpreter.invoke()
            
            output_data = interpreter.get_tensor(output_details[0]['index'])
            vec = output_data[0][0].tolist() 
            
            if len(vec) > 384: vec = vec[:384]
            while len(vec) < 384: vec.append(0.0)
            
            return vec
        
        # MOCK FALLBACK: Generates deterministic 384-dimensional vector
        np.random.seed(len(text))
        mock_vec = np.random.uniform(-0.1, 0.1, 384).tolist()
        return mock_vec

    def log_message(self, format, *args):
        pass

if __name__ == '__main__':
    server_address = ('127.0.0.1', 5001)
    httpd = HTTPServer(server_address, ReflexHandler)
    logging.info(f"Reflex Embedder Minion ignited on port {server_address[1]}")
    httpd.serve_forever()
