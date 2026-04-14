//! Red Queen Adversarial Test Suite for twerk-vam domain types
//!
//! Run with: cargo run -p twerk-core --example red_queen_adversarial

use std::str::FromStr;
use twerk_core::domain::{CronExpression, Hostname, WebhookUrl};

fn test_webhook_url(input: &str) -> bool {
    WebhookUrl::from_str(input).is_ok()
}

fn test_hostname(input: &str) -> bool {
    Hostname::from_str(input).is_ok()
}

fn test_cron(input: &str) -> bool {
    CronExpression::from_str(input).is_ok()
}

fn main() {
    println!("=== RED QUEEN ADVERSARIAL TEST SUITE ===\n");

    // ========================================================================
    // WEBHOOKURL ADVERSARIAL CASES
    // ========================================================================
    println!("--- WebhookUrl Adversarial Cases ---");

    // Build vectors first to avoid borrow issues
    let long_url = format!("https://example.com/{}", "x".repeat(2100));

    let webhook_cases: Vec<(&str, &str, bool)> = vec![
        // Very long URLs (>2048 chars)
        ("Very long URL (2100 char path)", &long_url, false),
        // URLs with unusual characters in path
        (
            "URL with spaces in path",
            "https://example.com/path with spaces",
            false,
        ),
        // URLs with special chars in path
        (
            "URL with special chars in path",
            "https://example.com/api/v1/users/!$&'()*+,;=:@",
            true,
        ),
        // URLs with IP addresses
        ("URL with IPv4", "http://192.168.1.1:8080/webhook", true),
        ("URL with localhost IP", "http://127.0.0.1:3000/", true),
        // IDN URLs (Non-goal)
        (
            "URL with international domain",
            "https://münchen.example.com/",
            false,
        ),
        // Mixed case schemes
        ("Mixed case HTTP", "HTTP://EXAMPLE.COM/", true),
        ("Mixed case HTTPS", "HTTPS://EXAMPLE.COM/", true),
        ("Mixed case Http", "Http://Example.Com/Path", true),
        // URLs with fragments
        (
            "URL with fragment",
            "https://example.com/page#section",
            true,
        ),
        // Data URLs (should be rejected)
        ("Data URL", "data:text/html,<h1>Hello</h1>", false),
        // Other non-http schemes (should be rejected)
        ("FTP scheme", "ftp://example.com/file", false),
        ("File scheme", "file:///etc/passwd", false),
        ("Mailto scheme", "mailto://user@example.com", false),
        // Empty/missing components
        ("URL with no host", "http://", false),
        ("URL scheme only", "https:", false),
        // Valid edge cases
        ("URL with just host", "https://example.com", true),
        ("URL with query", "https://example.com/?q=1", true),
    ];

    let mut webhook_slipped = 0;
    let mut webhook_caught = 0;
    for (name, input, should_pass) in webhook_cases {
        let passed = test_webhook_url(input);
        let caught = passed == should_pass;
        if caught {
            webhook_caught += 1;
        } else {
            webhook_slipped += 1;
        }
        let status = if caught { "✓ CAUGHT" } else { "✗ SLIPPED" };
        println!(
            "  [{}] {}: input='{}'",
            status,
            name,
            &input[..input.len().min(60)]
        );
        if !caught {
            if passed {
                println!("        -> Accepted but SHOULD reject");
            } else {
                println!("        -> Rejected but SHOULD accept");
            }
        }
    }
    println!(
        "  WebhookUrl: {} caught, {} slipped through",
        webhook_caught, webhook_slipped
    );

    // ========================================================================
    // HOSTNAME ADVERSARIAL CASES
    // ========================================================================
    println!("\n--- Hostname Adversarial Cases ---");

    let hostname_253 = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(61)
    );
    let hostname_254 = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(62)
    );
    let label_63 = format!("{}.com", "a".repeat(63));
    let label_64 = format!("{}.com", "a".repeat(64));

    let hostname_cases: Vec<(&str, &str, bool)> = vec![
        // Length boundaries
        ("253-char hostname (max)", &hostname_253, true),
        ("254-char hostname (over)", &hostname_254, false),
        // Label length boundaries
        ("63-char label (max)", &label_63, true),
        ("64-char label (over)", &label_64, false),
        // All-numeric labels (should be rejected)
        ("All-numeric labels", "123.456.789", false),
        ("Single numeric label", "123.example.com", false),
        // Labels starting/ending with hyphen
        ("Label starting with hyphen", "-host.example.com", false),
        ("Label ending with hyphen", "host-.example.com", false),
        // Unicode/IDN (Non-goal per NG5)
        ("Unicode hostname", "münchen.example.com", false),
        // Hostnames with ports (should be rejected)
        ("Hostname with port", "example.com:8080", false),
        ("IPv4-like with port", "192.168.1.1:8080", false),
        // Empty
        ("Empty hostname", "", false),
        // Valid cases
        ("Single char hostname", "a", true),
        ("Standard hostname", "api.example.com", true),
    ];

    let mut hostname_slipped = 0;
    let mut hostname_caught = 0;
    for (name, input, should_pass) in hostname_cases {
        let passed = test_hostname(input);
        let caught = passed == should_pass;
        if caught {
            hostname_caught += 1;
        } else {
            hostname_slipped += 1;
        }
        let status = if caught { "✓ CAUGHT" } else { "✗ SLIPPED" };
        println!("  [{}] {}: input='{}'", status, name, input);
        if !caught {
            if passed {
                println!("        -> Accepted but SHOULD reject");
            } else {
                println!("        -> Rejected but SHOULD accept");
            }
        }
    }
    println!(
        "  Hostname: {} caught, {} slipped through",
        hostname_caught, hostname_slipped
    );

    // ========================================================================
    // CRONEXPRESSION ADVERSARIAL CASES
    // ========================================================================
    println!("\n--- CronExpression Adversarial Cases ---");

    let cron_cases: Vec<(&str, &str, bool)> = vec![
        // Calendar edge cases (cron validates syntax, not calendar validity)
        ("February 30th", "0 0 30 2 *", true), // Cron accepts syntactically, calendar invalid
        ("February 29th", "0 0 29 2 *", true),
        ("November 31", "0 0 31 11 *", true),
        // Invalid field values
        ("Minute 60", "60 * * * *", false),
        ("Hour 25", "* 25 * * *", false),
        ("Day 32", "* * 32 * *", false),
        ("Month 13", "* * * 13 *", false),
        ("Day of week 8", "* * * * 8", false),
        // Invalid field counts
        ("4-field expression", "* * * *", false),
        ("7-field expression", "* * * * * * *", false),
        // Empty
        ("Empty expression", "", false),
        // Valid special syntax
        ("Range syntax", "0 9-17 * * 1-5", true),
        ("Step syntax", "*/15 * * * *", true),
        ("List syntax", "0 8,12,18 * * *", true),
        // Case insensitivity
        ("Lowercase day names", "0 0 * * mon-sun", true),
        ("Uppercase month names", "0 0 1 JAN *", true),
        ("Mixed case names", "0 0 * * Mon,Wed,Fri", true),
        // 6-field format
        ("6-field with seconds", "30 0 0 1 * *", true),
        ("6-field every 30 sec", "*/30 * * * * *", true),
    ];

    let mut cron_slipped = 0;
    let mut cron_caught = 0;
    for (name, input, should_pass) in cron_cases {
        let passed = test_cron(input);
        let caught = passed == should_pass;
        if caught {
            cron_caught += 1;
        } else {
            cron_slipped += 1;
        }
        let status = if caught { "✓ CAUGHT" } else { "✗ SLIPPED" };
        println!("  [{}] {}: input='{}'", status, name, input);
        if !caught {
            if passed {
                println!("        -> Accepted but SHOULD reject");
            } else {
                println!("        -> Rejected but SHOULD accept");
            }
        }
    }
    println!(
        "  CronExpression: {} caught, {} slipped through",
        cron_caught, cron_slipped
    );

    println!("\n=== SUMMARY ===");
    println!(
        "WebhookUrl:    {} caught, {} slipped",
        webhook_caught, webhook_slipped
    );
    println!(
        "Hostname:      {} caught, {} slipped",
        hostname_caught, hostname_slipped
    );
    println!(
        "CronExpression: {} caught, {} slipped",
        cron_caught, cron_slipped
    );

    let total = webhook_caught + hostname_caught + cron_caught;
    let total_slipped = webhook_slipped + hostname_slipped + cron_slipped;
    println!("TOTAL:         {} caught, {} slipped", total, total_slipped);

    if total_slipped > 0 {
        println!("\n⚠️  VULNERABILITIES FOUND - Contract violations possible!");
    } else {
        println!("\n✓  All adversarial cases handled correctly");
    }
    println!("\n=== END OF ADVERSARIAL TEST SUITE ===");
}
