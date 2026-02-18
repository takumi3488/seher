use chrono::{DateTime, Utc};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use seher::{BrowserDetector, BrowserType, ClaudeClient, CookieReader, UsageResponse};
use std::str::FromStr;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "seher",
    about = "CLI tool for Claude.ai rate limit monitoring",
    disable_help_flag = true
)]
pub struct Args {
    /// Browser to use (chrome, edge, brave, firefox, safari, etc.)
    #[arg(long, short)]
    pub browser: Option<String>,

    /// Browser profile name (e.g. "Profile 1", "default-release")
    #[arg(long, short)]
    pub profile: Option<String>,

    /// Arguments to pass to claude (options and/or prompt)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub claude_args: Vec<String>,
}

pub async fn run(args: Args) {
    let detector = BrowserDetector::new();
    let browsers = detector.detect_browsers();

    if browsers.is_empty() {
        eprintln!("No browsers found");
        return;
    }

    // Build list of (label, cookies) pairs to try
    let mut candidates: Vec<(String, Vec<seher::Cookie>)> = Vec::new();

    if let Some(ref browser_name) = args.browser {
        // Specific browser requested
        let browser_type = match BrowserType::from_str(browser_name) {
            Ok(bt) => bt,
            Err(e) => {
                eprintln!("{}", e);
                return;
            }
        };

        if !browsers.contains(&browser_type) {
            eprintln!("{} is not installed", browser_name);
            return;
        }

        if let Some(ref profile_name) = args.profile {
            // Specific profile
            let prof = detector.get_profile(browser_type, Some(profile_name));
            match prof {
                Some(p) => match CookieReader::read_cookies(&p, "claude.ai") {
                    Ok(cookies) => {
                        let label = format!("{} - {}", browser_type.name(), p.name);
                        candidates.push((label, cookies));
                    }
                    Err(e) => {
                        eprintln!("Failed to read cookies: {}", e);
                        return;
                    }
                },
                None => {
                    eprintln!(
                        "Profile '{}' not found for {}",
                        profile_name,
                        browser_type.name()
                    );
                    return;
                }
            }
        } else {
            // All profiles of specified browser
            for prof in detector.list_profiles(browser_type) {
                if let Ok(cookies) = CookieReader::read_cookies(&prof, "claude.ai")
                    && cookies.iter().any(|c| c.name == "sessionKey")
                {
                    let label = format!("{} - {}", browser_type.name(), prof.name);
                    candidates.push((label, cookies));
                }
            }
        }
    } else {
        // Auto-detect: scan all Chromium browsers
        for browser in &browsers {
            if !browser.is_chromium_based() {
                continue;
            }
            for prof in detector.list_profiles(*browser) {
                if let Ok(cookies) = CookieReader::read_cookies(&prof, "claude.ai")
                    && cookies.iter().any(|c| c.name == "sessionKey")
                {
                    let label = format!("{} - {}", browser.name(), prof.name);
                    candidates.push((label, cookies));
                }
            }
        }
    }

    if candidates.is_empty() {
        eprintln!("No claude.ai session cookies found");
        return;
    }

    for (label, cookies) in &candidates {
        println!("Trying {}...", label);
        match ClaudeClient::fetch_usage(cookies).await {
            Ok(usage) => {
                println!("Usage (via {}):", label);
                display_usage(&usage);

                match usage.next_reset_time() {
                    Some(reset_time) => sleep_until_reset(reset_time).await,
                    None => println!("\nUtilization is not at 100%, no sleep needed."),
                }

                // Launch Claude Code after waiting for rate limit reset
                let mut final_args = args.claude_args;
                if !has_prompt(&final_args) {
                    match prompt_from_editor().await {
                        Ok(prompt) if !prompt.is_empty() => final_args.push(prompt),
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Editor error: {}", e);
                            return;
                        }
                    }
                }

                std::process::Command::new("claude")
                    .args(&final_args)
                    .status()
                    .expect("claude command failed");
                return;
            }
            Err(e) => {
                eprintln!("  Failed: {}", e);
            }
        }
    }

    eprintln!("All profiles failed to fetch usage data");
}

fn has_prompt(args: &[String]) -> bool {
    args.iter().any(|a| !a.starts_with('-'))
}

async fn prompt_from_editor() -> std::result::Result<String, Box<dyn std::error::Error>> {
    let tmp = tempfile::NamedTempFile::new()?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    std::process::Command::new(&editor)
        .arg(tmp.path())
        .status()?;
    Ok(std::fs::read_to_string(tmp.path())?.trim().to_string())
}

fn display_usage(usage: &UsageResponse) {
    if let Some(w) = &usage.five_hour {
        println!(
            "  5-hour:         utilization={:.1}%, resets_at={}",
            w.utilization, w.resets_at
        );
    }
    if let Some(w) = &usage.seven_day {
        println!(
            "  7-day:          utilization={:.1}%, resets_at={}",
            w.utilization, w.resets_at
        );
    }
    if let Some(w) = &usage.seven_day_sonnet {
        println!(
            "  7-day (Sonnet): utilization={:.1}%, resets_at={}",
            w.utilization, w.resets_at
        );
    }
}

async fn sleep_until_reset(reset_time: DateTime<Utc>) {
    let now = Utc::now();
    if reset_time <= now {
        println!("\nReset time has already passed, no sleep needed.");
        return;
    }

    let total_secs = (reset_time - now).num_seconds().max(0) as u64;
    println!(
        "\nSleeping until {} ({} seconds)...",
        reset_time.format("%Y-%m-%d %H:%M:%S UTC"),
        total_secs
    );

    let pb = ProgressBar::new(total_secs);
    pb.set_style(
        ProgressStyle::with_template(
            "⏳ [{bar:40.cyan/blue}] {elapsed} elapsed, {eta} remaining ({pos}/{len}s)",
        )
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏ "),
    );

    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.tick().await; // first tick fires immediately
    loop {
        interval.tick().await;
        let remaining = (reset_time - Utc::now()).num_seconds().max(0) as u64;
        pb.set_position(total_secs - remaining);
        if remaining == 0 {
            break;
        }
    }
    pb.finish_with_message("Done! Reset time reached.");
}
