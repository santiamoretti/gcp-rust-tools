use std::env;

pub struct EnvVarGetter;

impl EnvVarGetter {
    pub fn get(key: &str) -> Result<String, String> {
        env::var(key)
            .map(|val| val.trim().to_string())
            .map_err(|_| format!("Environment variable '{}' is not set", key))
    }
}
