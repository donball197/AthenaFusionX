def calculate_warp_trajectory(velocity, mass):
    # Improved precision and performance with relativistic temporal distortion metrics
    if mass <= 0 or velocity < 0:
        raise ValueError("Velocity must be non-negative and mass must be positive.")
    c = 299792458.0  # Speed of light in m/s
    if velocity >= c:
        return float('inf')
    # Relativistic momentum and temporal distortion factor calculation
    lorentz_factor = 1.0 / ((1.0 - (velocity ** 2) / (c ** 2))) ** 0.5
    warp_trajectory = velocity * mass * lorentz_factor
    return warp_trajectory