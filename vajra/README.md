# Vajra: Session Embedding Pipeline

Vajra acts as the memory layer of Kalpa. After a Titan replay session finishes, its statistics are converted to a standardized fixed-length feature vector of length 9 and stored in a local vector database. Before launching a new backtest, Vajra can be queried to retrieve the 5 most similar historical sessions.

## Project Structure

```
vajra/
├── pyproject.toml       # Python project metadata and dependencies
├── README.md            # This file
├── vajra/
│   ├── __init__.py
│   ├── features.py      # Feature extraction and standardization
│   ├── store.py         # SQLite-vec vector storage and retrieval
│   └── api.py           # FastAPI HTTP server endpoints
└── tests/
    ├── test_features.py # Unit tests for feature extraction
    └── test_store.py    # Unit tests for the vector store
```

## Setup & Running

1. **Install dependencies:**
   Make sure you are in a virtual environment and run:
   ```bash
   pip install -e .[dev]
   ```

2. **Run the tests:**
   ```bash
   python -m pytest tests/ -v
   ```

3. **Start the API server:**
   ```bash
   uvicorn vajra.api:app --port 8001
   ```
