from fastapi import FastAPI, HTTPException, Query
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field
from typing import Optional, List, Dict, Any
import numpy as np

from vajra.features import extract_features
from vajra.store import VajraStore

app = FastAPI(
    title="Vajra API",
    description="The memory layer of Kalpa - Session Embedding pipeline"
)

# Initialize the vector store using 'vajra.db'
store = VajraStore("vajra.db")

class StatsModel(BaseModel):
    realized_vol: float
    spread_mean: float
    spread_max: float
    depth_imbalance: float
    momentum_5m: float
    momentum_30m: float
    volume_total: int
    fill_rate: float
    vwap_deviation: float

class MetricsModel(BaseModel):
    pnl: float
    sharpe: float
    max_dd: float
    win_rate: float
    strategy: str
    label: str

class SessionInsertRequest(BaseModel):
    session_id: str
    date: str
    instrument: str
    stats: StatsModel
    metrics: MetricsModel

class QueryRequest(BaseModel):
    stats: StatsModel
    k: int = Field(default=5, ge=1)

def get_dict(model: BaseModel) -> Dict[str, Any]:
    """Helper to convert Pydantic model to dictionary compatible with v1 and v2."""
    return model.model_dump() if hasattr(model, "model_dump") else model.dict()

@app.on_event("shutdown")
def shutdown_event():
    """Ensure database connection is closed properly on shutdown."""
    store.close()

@app.post("/sessions")
def create_session(request: SessionInsertRequest):
    """
    Standardize stats, store the session and its metrics in the database.
    """
    try:
        stats_dict = get_dict(request.stats)
        features = extract_features(stats_dict)
        metrics_dict = get_dict(request.metrics)
        
        store.store_session(
            session_id=request.session_id,
            date=request.date,
            instrument=request.instrument,
            features=features,
            metrics=metrics_dict
        )
        return {"stored": True, "session_id": request.session_id}
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))

@app.post("/query")
def query_sessions(request: QueryRequest):
    """
    Standardize target stats and search for top-k similar sessions.
    """
    try:
        stats_dict = get_dict(request.stats)
        features = extract_features(stats_dict)
        results = store.query_similar(features, k=request.k)
        return {"results": results}
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))

@app.get("/sessions")
def get_sessions(instrument: Optional[str] = None):
    """
    Retrieve all sessions, optionally filtered by instrument.
    """
    try:
        sessions = store.list_sessions(instrument=instrument)
        formatted_sessions = []
        for s in sessions:
            formatted_sessions.append({
                "session_id": s["session_id"],
                "date": s["date"],
                "instrument": s["instrument"],
                "metrics": s["metrics"],
                "features": s["features"].tolist()
            })
        return {"sessions": formatted_sessions}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@app.get("/sessions/{session_id}")
def get_session_by_id(session_id: str):
    """
    Get a single session's full details by its ID.
    Returns 404 if not found.
    """
    try:
        session = store.get_session(session_id)
        if session is None:
            return JSONResponse(status_code=404, content={"error": "not found"})
        
        return {
            "session_id": session["session_id"],
            "date": session["date"],
            "instrument": session["instrument"],
            "metrics": session["metrics"],
            "features": session["features"].tolist()
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
