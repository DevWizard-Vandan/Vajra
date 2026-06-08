import os
import numpy as np
import pytest
from vajra.store import VajraStore

@pytest.fixture
def test_store():
    # Use in-memory SQLite database for test isolation
    store = VajraStore(":memory:")
    yield store
    store.close()

def test_store_and_retrieve_session(test_store):
    features = np.array([0.1, -0.2, 0.3, 0.0, 0.5, -0.1, 0.2, -0.3, 0.4], dtype=np.float32)
    metrics = {
        "pnl": 1000.0,
        "sharpe": 1.5,
        "max_dd": -0.1,
        "win_rate": 0.55,
        "strategy": "mean_reversion",
        "label": "neutral"
    }
    
    # Store session
    test_store.store_session(
        session_id="session_1",
        date="2026-06-08",
        instrument="NIFTY",
        features=features,
        metrics=metrics
    )
    
    # Retrieve session
    session = test_store.get_session("session_1")
    assert session is not None
    assert session["session_id"] == "session_1"
    assert session["date"] == "2026-06-08"
    assert session["instrument"] == "NIFTY"
    assert session["metrics"] == metrics
    assert np.allclose(session["features"], features)

def test_query_similar_k(test_store):
    metrics = {
        "pnl": 1000.0, "sharpe": 1.5, "max_dd": -0.1,
        "win_rate": 0.55, "strategy": "mean_reversion", "label": "neutral"
    }
    
    for i in range(5):
        features = np.zeros(9, dtype=np.float32)
        features[i % 9] = 1.0  # simple orthogonal features
        test_store.store_session(
            session_id=f"session_{i}",
            date="2026-06-08",
            instrument="NIFTY",
            features=features,
            metrics=metrics
        )
        
    query_features = np.zeros(9, dtype=np.float32)
    query_features[0] = 1.0
    
    # query with k=3
    results = test_store.query_similar(query_features, k=3)
    assert len(results) == 3
    
    # query with k=5
    results_5 = test_store.query_similar(query_features, k=5)
    assert len(results_5) == 5

def test_similarity_score_bounds(test_store):
    features1 = np.array([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], dtype=np.float32)
    features2 = np.array([-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], dtype=np.float32)
    
    metrics = {
        "pnl": 1000.0, "sharpe": 1.5, "max_dd": -0.1,
        "win_rate": 0.55, "strategy": "mean_reversion", "label": "neutral"
    }
    
    test_store.store_session("s1", "2026-06-08", "NIFTY", features1, metrics)
    test_store.store_session("s2", "2026-06-08", "NIFTY", features2, metrics)
    
    # Query with features1
    results = test_store.query_similar(features1, k=2)
    
    for res in results:
        assert 0.0 <= res["similarity"] <= 1.0

def test_store_10_sessions_ranking(test_store):
    metrics = {
        "pnl": 1000.0, "sharpe": 1.5, "max_dd": -0.1,
        "win_rate": 0.55, "strategy": "mean_reversion", "label": "neutral"
    }
    
    # Store 10 sessions with different vectors.
    # Make session_7 identical to the query vector.
    query_vector = np.array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9], dtype=np.float32)
    
    for i in range(10):
        if i == 7:
            vec = query_vector.copy()
        else:
            vec = query_vector + (i + 1) * 2.0
            
        test_store.store_session(
            session_id=f"session_{i}",
            date="2026-06-08",
            instrument="NIFTY",
            features=vec,
            metrics=metrics
        )
        
    results = test_store.query_similar(query_vector, k=5)
    assert len(results) == 5
    
    # The identical session (session_7) must rank #1
    assert results[0]["session_id"] == "session_7"
    assert pytest.approx(results[0]["similarity"], abs=1e-5) == 1.0
