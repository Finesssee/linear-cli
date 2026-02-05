use colored::Colorize;

pub fn priority_to_string(priority: Option<i64>) -> String {
    match priority {
        Some(0) => "-".to_string(),
        Some(1) => "Urgent".red().to_string(),
        Some(2) => "High".yellow().to_string(),
        Some(3) => "Normal".to_string(),
        Some(4) => "Low".dimmed().to_string(),
        _ => "-".to_string(),
    }
}
