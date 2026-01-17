use std::process::Command;

const REPO_URL: &str = "https://github.com/CairoAC/finder.git";
const CARGO_TOML_URL: &str = "https://raw.githubusercontent.com/CairoAC/finder/master/Cargo.toml";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn run_update() {
    println!("Updating finder...");
    let status = Command::new("cargo")
        .args(["install", "--git", REPO_URL, "--force"])
        .status();

    match status {
        Ok(s) if s.success() => println!("Update complete!"),
        Ok(_) => println!("Update failed."),
        Err(e) => println!("Failed to run cargo: {}", e),
    }
}

pub async fn check_for_update() -> Option<String> {
    let response = reqwest::get(CARGO_TOML_URL).await.ok()?;
    let text = response.text().await.ok()?;

    for line in text.lines() {
        if line.starts_with("version") {
            let version = line
                .split('=')
                .nth(1)?
                .trim()
                .trim_matches('"');

            if version != current_version() {
                return Some(version.to_string());
            }
            break;
        }
    }
    None
}
