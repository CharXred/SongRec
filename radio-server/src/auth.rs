use anyhow::anyhow;
use std::env;

const AUTH_TOKEN_ENV: &str = "AUTH_TOKEN";

pub fn check(token: &str) -> anyhow::Result<()> {
    match env::var(AUTH_TOKEN_ENV) {
        Ok(v) => {
            if v != token {
                return Err(anyhow!("unauthorized"));
            }
        }
        Err(e) => return Err(anyhow!(e)),
    }
    Ok(())
}
