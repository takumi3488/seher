use chrono::{DateTime, Local, Utc};
use clap::Parser;
use seher::{Agent, AgentLimit, BrowserDetector, BrowserType, CookieReader, Settings};
use std::str::FromStr;
use zzsleep::sleep_until;

#[derive(Parser)]
#[command(
    name = "seher",
    about = "CLI tool for Claude.ai and Copilot rate limit monitoring",
    disable_help_flag = true
)]
pub struct Args {
    /// Browser to use (chrome, edge, brave, firefox, safari, etc.)
    #[arg(long, short)]
    pub browser: Option<String>,

    /// Browser profile name (e.g. "Profile 1", "default-release")
    #[arg(long, short)]
    pub profile: Option<String>,

    /// Suppress informational output (usage, sleep progress, etc.)
    #[arg(long, short)]
    pub quiet: bool,

    /// Additional arguments to pass to the agent command
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub agent_args: Vec<String>,
}

pub async fn run(args: Args) {
    let settings = match Settings::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to load settings: {}", e);
            return;
        }
    };

    let detector = BrowserDetector::new();
    let browsers = detector.detect_browsers();

    if browsers.is_empty() {
        eprintln!("No browsers found");
        return;
    }

    let mut agents: Vec<Agent> = Vec::new();

    for config in &settings.agents {
        let domain = get_domain_for_command(&config.command);
        
        let cookies = match get_cookies_for_domain(
            &detector,
            &browsers,
            &args.browser,
            &args.profile,
            domain,
        ) {
            Some(c) => c,
            None => {
                if !args.quiet {
                    eprintln!(
                        "No cookies found for {} (domain: {})",
                        config.command, domain
                    );
                }
                continue;
            }
        };

        agents.push(Agent::new(config.clone(), cookies, domain.to_string()));
    }

    if agents.is_empty() {
        eprintln!("No agents with valid cookies found");
        return;
    }

    let mut limit_results: Vec<(usize, Result<seher::AgentLimit, Box<dyn std::error::Error>>)> =
        Vec::new();

    for (i, agent) in agents.iter().enumerate() {
        if !args.quiet {
            println!(
                "Checking limit for {} {}...",
                agent.command(),
                agent.args().join(" ")
            );
        }

        let result = agent.check_limit().await;
        limit_results.push((i, result));
    }

    let mut available_indices: Vec<usize> = Vec::new();
    let mut limited_indices: Vec<(usize, Option<DateTime<Utc>>)> = Vec::new();

    for (i, result) in &limit_results {
        match result {
            Ok(AgentLimit::NotLimited) => {
                available_indices.push(*i);
            }
            Ok(AgentLimit::Limited { reset_time }) => {
                limited_indices.push((*i, *reset_time));
            }
            Err(e) => {
                if !args.quiet {
                    eprintln!("Failed to check limit for agent {}: {}", i, e);
                }
            }
        }
    }

    if !available_indices.is_empty() {
        let selected_index = available_indices[0];
        if !args.quiet {
            println!(
                "Agent {} is available (not limited)",
                agents[selected_index].command()
            );
        }
        execute_agent(&agents, selected_index, args.agent_args, args.quiet).await;
        return;
    }

    if !limited_indices.is_empty() {
        let mut earliest: Option<(usize, DateTime<Utc>)> = None;
        for (i, reset_time) in &limited_indices {
            if let Some(rt) = reset_time {
                if earliest.is_none() || *rt < earliest.unwrap().1 {
                    earliest = Some((*i, *rt));
                }
            }
        }

        if let Some((idx, rt)) = earliest {
            if !args.quiet {
                println!(
                    "All agents limited. Waiting for {} ({} seconds)...",
                    rt.format("%Y-%m-%d %H:%M:%S UTC"),
                    (rt - Utc::now()).num_seconds()
                );
            }
            sleep_until_reset(rt, args.quiet).await;
            execute_agent(&agents, idx, args.agent_args, args.quiet).await;
            return;
        } else if !args.quiet {
            println!("All agents limited, no reset time available");
        }
    }

    eprintln!("No available agents");
}

fn get_domain_for_command(command: &str) -> &str {
    match command {
        "claude" => "claude.ai",
        "copilot" => "github.com",
        _ => "claude.ai",
    }
}

fn get_cookies_for_domain(
    detector: &BrowserDetector,
    browsers: &[BrowserType],
    browser_arg: &Option<String>,
    profile_arg: &Option<String>,
    domain: &str,
) -> Option<Vec<seher::Cookie>> {
    if let Some(browser_name) = browser_arg {
        let browser_type = BrowserType::from_str(browser_name).ok()?;
        if !browsers.contains(&browser_type) {
            return None;
        }

        if let Some(profile_name) = profile_arg {
            let prof = detector.get_profile(browser_type, Some(profile_name))?;
            let cookies = CookieReader::read_cookies(&prof, domain).ok()?;
            if cookies.iter().any(|c| has_session_cookie(domain, c)) {
                return Some(cookies);
            }
            return None;
        }

        for prof in detector.list_profiles(browser_type) {
            if let Ok(cookies) = CookieReader::read_cookies(&prof, domain) {
                if cookies.iter().any(|c| has_session_cookie(domain, c)) {
                    return Some(cookies);
                }
            }
        }
        return None;
    }

    for browser in browsers {
        if !browser.is_chromium_based() {
            continue;
        }
        for prof in detector.list_profiles(*browser) {
            if let Ok(cookies) = CookieReader::read_cookies(&prof, domain) {
                if cookies.iter().any(|c| has_session_cookie(domain, c)) {
                    return Some(cookies);
                }
            }
        }
    }

    None
}

fn has_session_cookie(domain: &str, cookie: &seher::Cookie) -> bool {
    match domain {
        "claude.ai" => cookie.name == "sessionKey",
        "github.com" => cookie.name == "dotcom_user" || cookie.name == "logged_in",
        _ => false,
    }
}

async fn prompt_from_editor() -> std::result::Result<String, Box<dyn std::error::Error>> {
    let tmp = tempfile::NamedTempFile::new()?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    std::process::Command::new(&editor)
        .arg(tmp.path())
        .status()?;
    Ok(std::fs::read_to_string(tmp.path())?.trim().to_string())
}

async fn execute_agent(
    agents: &[Agent],
    selected_index: usize,
    agent_args: Vec<String>,
    quiet: bool,
) {
    let selected_agent = &agents[selected_index];
    let mut final_args = agent_args.clone();

    if final_args.is_empty() && !quiet {
        match prompt_from_editor().await {
            Ok(prompt) if !prompt.is_empty() => final_args.push(prompt),
            Ok(_) => {}
            Err(e) => {
                eprintln!("Editor error: {}", e);
                return;
            }
        }
    }

    if !quiet {
        println!(
            "Executing: {} {}",
            selected_agent.command(),
            selected_agent
                .args()
                .iter()
                .chain(final_args.iter())
                .map(|s: &String| s.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
    }

    selected_agent.execute(final_args);
}

async fn sleep_until_reset(reset_time: DateTime<Utc>, quiet: bool) {
    let now = Utc::now();
    if reset_time <= now {
        if !quiet {
            println!("\nReset time has already passed, no sleep needed.");
        }
        return;
    }

    let total_secs = (reset_time - now).num_seconds().max(0) as u64;
    if !quiet {
        println!(
            "\nSleeping until {} ({} seconds)...",
            reset_time.format("%Y-%m-%d %H:%M:%S UTC"),
            total_secs
        );
    }

    let local_reset_time = reset_time.with_timezone(&Local);
    sleep_until(local_reset_time, quiet).await;
}
