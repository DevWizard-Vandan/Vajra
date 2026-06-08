import numpy as np

# Module-level constants representing reference (mean, std) for NIFTY 50 ranges.
FEATURE_STATS = {
    "realized_vol": (0.15, 0.05),
    "spread_mean": (1.0, 0.5),
    "spread_max": (3.0, 1.5),
    "depth_imbalance": (0.0, 0.3),
    "momentum_5m": (0.0, 0.002),
    "momentum_30m": (0.0, 0.005),
    "volume_total": (500000.0, 200000.0),
    "fill_rate": (0.5, 0.2),
    "vwap_deviation": (0.0, 0.0015),
}

FEATURE_KEYS = [
    "realized_vol",
    "spread_mean",
    "spread_max",
    "depth_imbalance",
    "momentum_5m",
    "momentum_30m",
    "volume_total",
    "fill_rate",
    "vwap_deviation",
]

def extract_features(session_stats: dict) -> np.ndarray:
    """
    Convert raw session statistics into a standardized z-score feature vector.
    
    Args:
        session_stats (dict): A dictionary containing the 9 raw feature keys.
        
    Returns:
        np.ndarray: A standardized 1D numpy array of shape (9,) and type float32.
    """
    arr = []
    for key in FEATURE_KEYS:
        val = float(session_stats[key])
        mean, std = FEATURE_STATS[key]
        z = (val - mean) / std if std > 0 else 0.0
        arr.append(z)
    return np.array(arr, dtype=np.float32)

def features_to_dict(arr: np.ndarray) -> dict:
    """
    Convert a standardized feature vector back to a dictionary of raw statistics.
    
    Args:
        arr (np.ndarray): Standardized feature vector of shape (9,).
        
    Returns:
        dict: Reconstructed raw statistics dictionary.
    """
    d = {}
    for i, key in enumerate(FEATURE_KEYS):
        z = float(arr[i])
        mean, std = FEATURE_STATS[key]
        val = z * std + mean
        if key == "volume_total":
            d[key] = int(round(val))
        else:
            d[key] = val
    return d

def dict_to_features(d: dict) -> np.ndarray:
    """
    Convert a dictionary of raw session statistics to a standardized feature vector.
    
    Args:
        d (dict): Raw session statistics.
        
    Returns:
        np.ndarray: Standardized feature vector.
    """
    return extract_features(d)
