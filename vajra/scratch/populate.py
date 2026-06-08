import urllib.request
import json

base_url = "http://localhost:8001"

sessions_data = [
    {
        "session_id": f"session_{i}",
        "date": "2026-06-08",
        "instrument": "NIFTY",
        "stats": {
            "realized_vol": 0.12 + i * 0.02,
            "spread_mean": 0.4 + i * 0.1,
            "spread_max": 1.0 + i * 0.3,
            "depth_imbalance": -0.2 + i * 0.1,
            "momentum_5m": 0.001 * i,
            "momentum_30m": -0.001 * i,
            "volume_total": 200000 + i * 50000,
            "fill_rate": 0.4 + i * 0.05,
            "vwap_deviation": 0.0002 * i
        },
        "metrics": {
            "pnl": 1000.0 * (i - 2),
            "sharpe": 0.5 * i,
            "max_dd": -0.02 * i,
            "win_rate": 0.45 + i * 0.03,
            "strategy": "mean_reversion" if i % 2 == 0 else "trend_following",
            "label": "bullish" if i > 2 else "bearish"
        }
    }
    for i in range(5)
]

print("Populating 5 sessions...")
for s in sessions_data:
    req = urllib.request.Request(
        f"{base_url}/sessions",
        data=json.dumps(s).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    with urllib.request.urlopen(req) as res:
        print(f"Stored {s['session_id']}: {res.read().decode()}")

# List sessions to verify
req = urllib.request.Request(f"{base_url}/sessions", method="GET")
with urllib.request.urlopen(req) as res:
    print("\nALL SESSIONS:")
    print(json.dumps(json.loads(res.read().decode()), indent=2))

# Query similar
query_body = {
    "stats": {
        "realized_vol": 0.16,
        "spread_mean": 0.6,
        "spread_max": 1.6,
        "depth_imbalance": 0.0,
        "momentum_5m": 0.002,
        "momentum_30m": -0.002,
        "volume_total": 300000,
        "fill_rate": 0.5,
        "vwap_deviation": 0.0004
    },
    "k": 5
}
req = urllib.request.Request(
    f"{base_url}/query",
    data=json.dumps(query_body).encode("utf-8"),
    headers={"Content-Type": "application/json"},
    method="POST"
)
with urllib.request.urlopen(req) as res:
    print("\nQUERY RESULTS:")
    print(json.dumps(json.loads(res.read().decode()), indent=2))
