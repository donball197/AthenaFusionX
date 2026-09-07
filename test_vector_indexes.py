#!/usr/bin/env python3
"""
Test script: Turbovec vs Qdrant
"""

import time
import numpy as np
from typing import List

EMBEDDING_DIM = 384
USE_REAL_EMBEDDINGS = False   # ← set to True when you want real Athena embeddings
TOP_K = 3

SAMPLE_DOCS = [
    "The ancient Egyptians built pyramids as tombs for their pharaohs using limestone and granite blocks.",
    "Machine learning is a subset of artificial intelligence that enables systems to learn from data.",
    "The Great Wall of China was constructed over many centuries to protect the Chinese empire from invaders.",
    "Python is a versatile programming language used for web development, data science, and AI applications.",
    "Photosynthesis is the process by which plants convert sunlight into chemical energy.",
    "The Roman Empire was one of the largest empires in history, spanning Europe, North Africa, and the Middle East.",
    "Deep learning uses neural networks with many layers to process complex data patterns.",
    "The Amazon River is the largest river in the world by volume, flowing through South America.",
    "Natural language processing helps computers understand, interpret, and respond to human language.",
    "The pyramids at Giza are the oldest of the Seven Wonders of the Ancient World."
]

QUERIES = [
    "What were the pyramids used for?",
    "Tell me about machine learning and AI",
    "largest river in the world",
    "Roman Empire history"
]


def get_embedding(text: str) -> List[float]:
    if USE_REAL_EMBEDDINGS:
        import requests, os
        url = os.getenv("ATHENA_EMBEDDING_URL", "http://localhost:9380/v1/embeddings")
        model = os.getenv("ATHENA_EMBEDDING_MODEL", "granite-311m-edge")
        resp = requests.post(url, json={"input": text, "model": model}, timeout=30)
        resp.raise_for_status()
        return resp.json()["data"][0]["embedding"]
    else:
        rng = np.random.RandomState(abs(hash(text)) % (2**32))
        vec = rng.randn(EMBEDDING_DIM).astype(np.float32)
        vec /= np.linalg.norm(vec) + 1e-9
        return vec.tolist()


def test_turbovec():
    print("\n" + "="*60)
    print("Testing Turbovec (TurboQuant)")
    print("="*60)

    try:
        from turbovec import TurboQuantIndex
    except ImportError:
        print("turbovec not installed")
        return

    index = TurboQuantIndex(dim=EMBEDDING_DIM, bit_width=4)

    start = time.perf_counter()
    vectors = [get_embedding(doc) for doc in SAMPLE_DOCS]
    index.add(np.array(vectors, dtype=np.float32))
    print(f"Indexed {len(SAMPLE_DOCS)} documents in {(time.perf_counter()-start)*1000:.1f} ms")

    for query in QUERIES:
        q_emb = np.array(get_embedding(query), dtype=np.float32).reshape(1, -1)

        start = time.perf_counter()
        scores, indices = index.search(q_emb, k=TOP_K)
        search_time = (time.perf_counter() - start) * 1000

        print(f"\nQuery: {query}")
        print(f"Search time: {search_time:.2f} ms")
        for rank, (score, idx) in enumerate(zip(scores[0], indices[0]), 1):
            print(f"  {rank}. score={score:.4f} → {SAMPLE_DOCS[idx][:75]}...")


def test_qdrant():
    print("\n" + "="*60)
    print("Testing Qdrant (in-memory)")
    print("="*60)

    try:
        from qdrant_client import QdrantClient
        from qdrant_client.http import models
    except ImportError:
        print("qdrant-client not installed")
        return

    client = QdrantClient(location=":memory:")

    # New recommended way (avoids deprecation warning)
    if client.collection_exists("test"):
        client.delete_collection("test")

    client.create_collection(
        collection_name="test",
        vectors_config=models.VectorParams(size=EMBEDDING_DIM, distance=models.Distance.COSINE),
    )

    start = time.perf_counter()
    points = [
        models.PointStruct(id=i, vector=get_embedding(doc), payload={"text": doc})
        for i, doc in enumerate(SAMPLE_DOCS)
    ]
    client.upsert(collection_name="test", points=points)
    print(f"Indexed {len(SAMPLE_DOCS)} documents in {(time.perf_counter()-start)*1000:.1f} ms")

    for query in QUERIES:
        q_emb = get_embedding(query)

        start = time.perf_counter()
        # New API (qdrant-client >= 1.10+)
        results = client.query_points(
            collection_name="test",
            query=q_emb,
            limit=TOP_K,
        ).points
        search_time = (time.perf_counter() - start) * 1000

        print(f"\nQuery: {query}")
        print(f"Search time: {search_time:.2f} ms")
        for rank, hit in enumerate(results, 1):
            print(f"  {rank}. score={hit.score:.4f} → {hit.payload['text'][:75]}...")


if __name__ == "__main__":
    print("Vector Index Test")
    print(f"Dimension: {EMBEDDING_DIM} | Real embeddings: {USE_REAL_EMBEDDINGS}")
    test_turbovec()
    test_qdrant()
    print("\nDone!")
