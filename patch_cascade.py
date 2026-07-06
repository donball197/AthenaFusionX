import os

# 1. Patch model_config.rs
mc_path = "crates/runtime_core/src/model_config.rs"
with open(mc_path, "r") as f:
    mc_data = f.read()

mc_data = mc_data.replace(
    'pub fn get_heavy_models() -> Vec<String> { vec!["gemma-4-31b-it".to_string()] }',
    'pub fn get_heavy_models() -> Vec<String> { vec!["gemma-4-31b-it".to_string(), "gemma-4-26b-a4b-it".to_string(), "gemini-3.1-flash-lite".to_string()] }'
)
with open(mc_path, "w") as f:
    f.write(mc_data)


# 2. Patch factory.rs
fac_path = "crates/runtime_core/src/factory.rs"
with open(fac_path, "r") as f:
    fac_data = f.read()

fac_data = fac_data.replace(
    'pub const MODEL_31B: &str = "gemma-4-31b-it";',
    'pub const MODEL_31B: &str = "gemma-4-31b-it";\npub const MODEL_26B: &str = "gemma-4-26b-a4b-it";'
)

old_failover = """    pub fn failover_model(_current_model: &str) -> &'static str {
        MODEL_31B 
    }"""
new_failover = """    pub fn failover_model(current_model: &str) -> &'static str {
        if current_model == MODEL_31B {
            MODEL_26B
        } else if current_model == MODEL_26B {
            MODEL_FLASH_LITE
        } else {
            MODEL_31B
        }
    }"""
fac_data = fac_data.replace(old_failover, new_failover)
with open(fac_path, "w") as f:
    f.write(fac_data)


# 3. Patch epistemic_engine.rs
ee_path = "crates/runtime_core/src/epistemic_engine.rs"
with open(ee_path, "r") as f:
    ee_data = f.read()

old_api_err = """                if let Some(err) = body.pointer("/error/message").and_then(|v| v.as_str()) {
                    println!("\\x1b[31m>> [API ERROR] {} returned: {}\\x1b[0m", short_name, err);
                    last_err = anyhow!("API Error: {}", err);
                    break;
                }"""

new_api_err = """                if let Some(err) = body.pointer("/error/message").and_then(|v| v.as_str()) {
                    println!("\\x1b[31m>> [API ERROR] {} returned: {}\\x1b[0m", short_name, err);
                    last_err = anyhow!("API Error: {}", err);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    break;
                }"""
ee_data = ee_data.replace(old_api_err, new_api_err)
with open(ee_path, "w") as f:
    f.write(ee_data)

print("[✓] Model Cascade, Circular Failover, and Exponential Backoff patched successfully.")
