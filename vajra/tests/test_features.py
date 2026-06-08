import numpy as np
import pytest
from vajra.features import extract_features, features_to_dict, dict_to_features

def test_extract_features_shape():
    stats = {
        "realized_vol": 0.18,
        "spread_mean": 0.5,
        "spread_max": 1.2,
        "depth_imbalance": 0.1,
        "momentum_5m": 0.003,
        "momentum_30m": -0.002,
        "volume_total": 250000,
        "fill_rate": 0.6,
        "vwap_deviation": 0.0005
    }
    arr = extract_features(stats)
    assert isinstance(arr, np.ndarray)
    assert arr.shape == (9,)
    assert arr.dtype == np.float32

def test_extract_features_finite():
    stats = {
        "realized_vol": 0.18,
        "spread_mean": 0.5,
        "spread_max": 1.2,
        "depth_imbalance": 0.1,
        "momentum_5m": 0.003,
        "momentum_30m": -0.002,
        "volume_total": 250000,
        "fill_rate": 0.6,
        "vwap_deviation": 0.0005
    }
    arr = extract_features(stats)
    assert np.all(np.isfinite(arr))

def test_boundary_values():
    # Zero values
    stats_zero = {
        "realized_vol": 0.0,
        "spread_mean": 0.0,
        "spread_max": 0.0,
        "depth_imbalance": 0.0,
        "momentum_5m": 0.0,
        "momentum_30m": 0.0,
        "volume_total": 0,
        "fill_rate": 0.0,
        "vwap_deviation": 0.0
    }
    arr = extract_features(stats_zero)
    assert arr.shape == (9,)
    assert np.all(np.isfinite(arr))

    # Large values
    stats_large = {
        "realized_vol": 10.0,
        "spread_mean": 1000.0,
        "spread_max": 10000.0,
        "depth_imbalance": 1.0,
        "momentum_5m": 5.0,
        "momentum_30m": 10.0,
        "volume_total": 99999999,
        "fill_rate": 1.0,
        "vwap_deviation": 0.5
    }
    arr_large = extract_features(stats_large)
    assert arr_large.shape == (9,)
    assert np.all(np.isfinite(arr_large))

    # Negative values
    stats_neg = {
        "realized_vol": 0.1,
        "spread_mean": 0.1,
        "spread_max": 0.1,
        "depth_imbalance": -1.0,
        "momentum_5m": -0.05,
        "momentum_30m": -0.1,
        "volume_total": 100,
        "fill_rate": 0.0,
        "vwap_deviation": -0.01
    }
    arr_neg = extract_features(stats_neg)
    assert arr_neg.shape == (9,)
    assert np.all(np.isfinite(arr_neg))

def test_inverse_mapping():
    stats = {
        "realized_vol": 0.18,
        "spread_mean": 0.5,
        "spread_max": 1.2,
        "depth_imbalance": 0.1,
        "momentum_5m": 0.003,
        "momentum_30m": -0.002,
        "volume_total": 250000,
        "fill_rate": 0.6,
        "vwap_deviation": 0.0005
    }
    arr = extract_features(stats)
    reconstructed = features_to_dict(arr)
    
    for key in stats:
        if key == "volume_total":
            assert abs(reconstructed[key] - stats[key]) <= 1
        else:
            assert pytest.approx(reconstructed[key], abs=1e-6) == stats[key]

    arr_from_dict = dict_to_features(stats)
    assert np.allclose(arr, arr_from_dict)
