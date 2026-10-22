use anyhow::{Result, bail};

/// Exact application identifiers, never process paths or executable commands.
pub fn parse_exclusions(value: &str) -> Result<Vec<String>> {
    if value.len() > 32 * 1024 {
        bail!("Application exclusion list exceeds 32 KiB");
    }
    let names: Vec<String> = serde_json::from_str(value)?;
    if names.len() > 100 {
        bail!("At most 100 application exclusions are supported");
    }
    for name in &names {
        if name.trim().is_empty()
            || name != name.trim()
            || name.chars().count() > 128
            || name.chars().any(char::is_control)
        {
            bail!(
                "Application names must contain 1–128 characters without surrounding whitespace or control characters"
            );
        }
    }
    Ok(names)
}
