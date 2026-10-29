#!/usr/bin/env python3
"""
AthenaFusionX Edge RAG Demo
Embeddings: Lenovo Duet (LiteRT granite-311m-edge) @ 127.0.0.1:9380
LLM: S23 Edge Node (LiteRT-LM gemma-4-e4b-it) @ 192.168.4.223:9379
"""

import requests
import json
import numpy as np

# --- Endpoints from your verified preflight ---
# If running inside Debian on the phone, use 127.0.0.1 for both.
# If running from the Chromebook, use 127.0.0.1 for embeddings and 192.168.4.223 for LLM.
EMBEDDING_URL = "http://127.0.0.1:9380/v1/embeddings"
LLM_URL = "http://192.168.4.223:9379/v1/chat/completions"

EMBEDDING_MODEL = "granite-311m-edge"
LLM_MODEL = "gemma-4-e4b-it"


class EdgeRAG:
    def __init__(self):
        self.documents = []
        self.embeddings = []

    def get_embedding(self, text: str):
        try:
            res = requests.post(
                EMBEDDING_URL,
                json={"input": text, "model": EMBEDDING_MODEL},
                timeout=5
            )
            if res.status_code == 200:
                vec = np.array(res.json()['data'][0]['embedding'], dtype=np.float32)
                # Ensure unit vector for fast dot-product similarity
                norm = np.linalg.norm(vec)
                return vec / norm if norm > 0 else vec
        except Exception as e:
            print(f"[!] Embedding error: {e}")
        return None

    def add_document(self, text: str):
        emb = self.get_embedding(text)
        if emb is not None:
            self.documents.append(text)
            self.embeddings.append(emb)
            return True
        return False

    def retrieve_relevant(self, query: str, top_k: int = 2):
        query_vec = self.get_embedding(query)
        if query_vec is None or not self.embeddings:
            return []

        doc_matrix = np.vstack(self.embeddings)  # Shape: (N, 384)
        # Cosine similarity via dot product on normalized vectors
        similarities = np.dot(doc_matrix, query_vec)

        # Top K ranking
        top_indices = np.argsort(similarities)[-top_k:][::-1]
        results = []
        for idx in top_indices:
            results.append((self.documents[idx], float(similarities[idx])))
        return results

    def ask_llm(self, query: str, context: str):
        prompt = (
            "You are a helpful assistant. Answer the question based ONLY on the provided context.\n\n"
            f"Context:\n{context}\n\n"
            f"Question: {query}\n\n"
            "Answer:"
        )

        payload = {
            "model": LLM_MODEL,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.2,
            "max_tokens": 256
        }

        try:
            res = requests.post(LLM_URL, json=payload, timeout=30)
            if res.status_code == 200:
                data = res.json()
                return data["choices"][0]["message"]["content"].strip()
            else:
                return f"[Error HTTP {res.status_code}]: {res.text}"
        except requests.exceptions.RequestException as e:
            return f"[Connection Failed]: {e}"

    def query(self, question: str):
        print(f"\n🔍 Query: \"{question}\"")
        matches = self.retrieve_relevant(question, top_k=2)

        if not matches:
            return "No relevant context found."

        print("\n--- Retrieved Context ---")
        context_lines = []
        for i, (doc, score) in enumerate(matches, 1):
            print(f"[{i}] (Score: {score:.4f}) {doc}")
            context_lines.append(doc)

        context_text = "\n".join(context_lines)

        print("\n🤖 Generating answer via Edge LLM...")
        return self.ask_llm(question, context_text)


if __name__ == "__main__":
    rag = EdgeRAG()

    knowledge_base = [
        "AthenaFusionX runs on edge devices like the Lenovo Duet with ARM architecture.",
        "The embedding model produces 384-dimensional vectors using the LiteRT engine.",
        "The primary reasoning model on the S23 is gemma-4-e4b-it running on LiteRT-LM.",
        "Cloud escalation routes complex reasoning to NVIDIA NIM and Google Gemini APIs.",
        "Semantic search uses cosine similarity over dense vector embeddings."
    ]

    print("Indexing knowledge base into vector memory...")
    for doc in knowledge_base:
        rag.add_document(doc)
    print(f"Indexed {len(knowledge_base)} documents successfully.")

    # Test query
    sample_question = "What hardware architecture and engine does the embedding model use?"
    answer = rag.query(sample_question)

    print("\n--- Final Answer ---")
    print(answer)
