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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_none() {
        assert_eq!(priority_to_string(None), "-");
    }

    #[test]
    fn test_priority_zero() {
        assert_eq!(priority_to_string(Some(0)), "-");
    }

    #[test]
    fn test_priority_urgent() {
        let result = priority_to_string(Some(1));
        assert!(result.contains("Urgent"));
    }

    #[test]
    fn test_priority_high() {
        let result = priority_to_string(Some(2));
        assert!(result.contains("High"));
    }

    #[test]
    fn test_priority_normal() {
        assert_eq!(priority_to_string(Some(3)), "Normal");
    }

    #[test]
    fn test_priority_low() {
        let result = priority_to_string(Some(4));
        assert!(result.contains("Low"));
    }

    #[test]
    fn test_priority_invalid() {
        assert_eq!(priority_to_string(Some(5)), "-");
        assert_eq!(priority_to_string(Some(-1)), "-");
    }
}
