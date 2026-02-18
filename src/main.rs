use seher::{BrowserDetector, CookieReader};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let detector = BrowserDetector::new();

    println!("Detecting installed browsers...");
    let browsers = detector.detect_browsers();

    if browsers.is_empty() {
        eprintln!("No Chromium-based browsers found");
        return;
    }

    println!("Found browsers:");
    for browser in &browsers {
        let profiles = detector.list_profiles(*browser);
        println!("  {} - {} profile(s)", browser.name(), profiles.len());
        for profile in profiles {
            println!("    - {}", profile.name);
        }
    }

    let browser = browsers.first().unwrap();
    let profile = detector.get_profile(*browser, None);

    match profile {
        Some(prof) => {
            println!("\nUsing {} - Profile: {}", browser.name(), prof.name);
            println!("Cookies path: {:?}", prof.cookies_path());

            match CookieReader::read_cookies(&prof, "claude.ai") {
                Ok(cookies) => {
                    println!("\nFound {} cookies for claude.ai:", cookies.len());
                    for cookie in cookies {
                        let value_preview = if cookie.value.len() > 20 {
                            format!("{}...", &cookie.value[..20])
                        } else {
                            cookie.value.clone()
                        };
                        println!(
                            "  - {} = {} (domain: {}, secure: {}, httponly: {})",
                            cookie.name,
                            value_preview,
                            cookie.domain,
                            cookie.is_secure,
                            cookie.is_httponly
                        );
                    }
                }
                Err(e) => {
                    eprintln!("\nFailed to read cookies: {}", e);
                }
            }
        }
        None => {
            eprintln!("No profile found for {}", browser.name());
        }
    }
}
