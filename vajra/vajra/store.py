import sqlite3
import sqlite_vec
import json
import numpy as np

class VajraStore:
    def __init__(self, db_path: str = "vajra.db"):
        """
        Create or open the SQLite database and load the sqlite-vec extension.
        
        Args:
            db_path (str): Path to the SQLite database file.
        """
        self.db_path = db_path
        self.conn = sqlite3.connect(db_path, check_same_thread=False)
        self.conn.enable_load_extension(True)
        sqlite_vec.load(self.conn)
        self.conn.enable_load_extension(False)
        self._create_tables()

    def _create_tables(self) -> None:
        """Create metadata and vector virtual tables if they do not exist."""
        # Standard table for session metadata
        self.conn.execute("""
            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT UNIQUE NOT NULL,
                date TEXT NOT NULL,
                instrument TEXT NOT NULL,
                metrics TEXT NOT NULL
            )
        """)
        
        # Virtual table for vector storage using cosine distance
        self.conn.execute("""
            CREATE VIRTUAL TABLE IF NOT EXISTS session_vectors USING vec0(
                embedding float[9] distance_metric=cosine
            )
        """)
        self.conn.commit()

    def store_session(
        self,
        session_id: str,
        date: str,           # "YYYY-MM-DD"
        instrument: str,
        features: np.ndarray,
        metrics: dict        # {"pnl": float, "sharpe": float, "max_dd": float,
                             #  "win_rate": float, "strategy": str, "label": str}
    ) -> None:
        """Store a session embedding + metadata. Overwrites if session_id exists."""
        # Remove any existing session with this session_id to allow updates/overwrites
        row = self.conn.execute("SELECT id FROM sessions WHERE session_id = ?", (session_id,)).fetchone()
        if row:
            internal_id = row[0]
            self.conn.execute("DELETE FROM sessions WHERE id = ?", (internal_id,))
            self.conn.execute("DELETE FROM session_vectors WHERE rowid = ?", (internal_id,))

        # Insert metadata
        cursor = self.conn.execute(
            "INSERT INTO sessions (session_id, date, instrument, metrics) VALUES (?, ?, ?, ?)",
            (session_id, date, instrument, json.dumps(metrics))
        )
        internal_id = cursor.lastrowid
        
        # Insert standardized vector matching the internal_id as rowid
        # Convert array to float32 first
        features_f32 = np.asarray(features, dtype=np.float32)
        serialized_vector = sqlite_vec.serialize_float32(features_f32)
        
        self.conn.execute(
            "INSERT INTO session_vectors (rowid, embedding) VALUES (?, ?)",
            (internal_id, serialized_vector)
        )
        self.conn.commit()

    def query_similar(
        self,
        query_features: np.ndarray,
        k: int = 5
    ) -> list[dict]:
        """
        Return the k most similar sessions.
        Each result: {
          "session_id": str,
          "date": str,
          "instrument": str,
          "similarity": float,   # cosine similarity, 0.0 to 1.0
          "metrics": dict
        }
        """
        query_f32 = np.asarray(query_features, dtype=np.float32)
        serialized_query = sqlite_vec.serialize_float32(query_f32)
        
        # Query nearest neighbors using the MATCH operator on the virtual table
        cursor = self.conn.execute(
            """
            WITH vec_matches AS (
                SELECT rowid, distance
                FROM session_vectors
                WHERE embedding MATCH ? AND k = ?
            )
            SELECT s.session_id, s.date, s.instrument, s.metrics, v.distance
            FROM vec_matches v
            JOIN sessions s ON v.rowid = s.id
            ORDER BY v.distance ASC
            """,
            (serialized_query, k)
        )
        
        results = []
        for row in cursor.fetchall():
            distance = float(row[4])
            # cosine similarity = 1.0 - distance
            similarity = 1.0 - distance
            # Bound similarity between 0.0 and 1.0
            similarity = max(0.0, min(1.0, similarity))
            
            results.append({
                "session_id": row[0],
                "date": row[1],
                "instrument": row[2],
                "similarity": similarity,
                "metrics": json.loads(row[3])
            })
        return results

    def get_session(self, session_id: str) -> dict | None:
        """Get full metadata for a session by ID."""
        row = self.conn.execute(
            """
            SELECT s.session_id, s.date, s.instrument, s.metrics, v.embedding
            FROM sessions s
            JOIN session_vectors v ON s.id = v.rowid
            WHERE s.session_id = ?
            """,
            (session_id,)
        ).fetchone()
        
        if not row:
            return None
        
        embedding_blob = row[4]
        # Reconstruct numpy array from the stored binary BLOB
        features = np.frombuffer(embedding_blob, dtype=np.float32)
        
        return {
            "session_id": row[0],
            "date": row[1],
            "instrument": row[2],
            "metrics": json.loads(row[3]),
            "features": features
        }

    def list_sessions(self, instrument: str | None = None) -> list[dict]:
        """List all sessions, optionally filtered by instrument."""
        if instrument:
            cursor = self.conn.execute(
                """
                SELECT s.session_id, s.date, s.instrument, s.metrics, v.embedding
                FROM sessions s
                JOIN session_vectors v ON s.id = v.rowid
                WHERE s.instrument = ?
                """,
                (instrument,)
            )
        else:
            cursor = self.conn.execute(
                """
                SELECT s.session_id, s.date, s.instrument, s.metrics, v.embedding
                FROM sessions s
                JOIN session_vectors v ON s.id = v.rowid
                """
            )
            
        results = []
        for row in cursor.fetchall():
            embedding_blob = row[4]
            features = np.frombuffer(embedding_blob, dtype=np.float32)
            results.append({
                "session_id": row[0],
                "date": row[1],
                "instrument": row[2],
                "metrics": json.loads(row[3]),
                "features": features
            })
        return results

    def close(self) -> None:
        """Close the database connection."""
        self.conn.close()
