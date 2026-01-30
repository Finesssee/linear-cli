use chrono::{Datelike, Duration, Local, NaiveDate};

/// Parse due date shorthand into an ISO date string (YYYY-MM-DD)
///
/// Supports:
/// - "today", "tomorrow", "yesterday"
/// - "+Nd" for N days from now (e.g., "+3d", "+7d")
/// - "-Nd" for N days ago
/// - "+Nw" for N weeks from now
/// - ISO date (YYYY-MM-DD) passthrough
/// - Common formats like "2024-01-15", "01/15/2024", "Jan 15"
pub fn parse_due_date(input: &str) -> Option<String> {
    let input = input.trim().to_lowercase();
    let today = Local::now().date_naive();

    // Handle special keywords
    match input.as_str() {
        "today" => return Some(format_date(today)),
        "tomorrow" | "tom" => return Some(format_date(today + Duration::days(1))),
        "yesterday" => return Some(format_date(today - Duration::days(1))),
        "monday" | "mon" => return Some(format_date(next_weekday(today, 0))),
        "tuesday" | "tue" => return Some(format_date(next_weekday(today, 1))),
        "wednesday" | "wed" => return Some(format_date(next_weekday(today, 2))),
        "thursday" | "thu" => return Some(format_date(next_weekday(today, 3))),
        "friday" | "fri" => return Some(format_date(next_weekday(today, 4))),
        "saturday" | "sat" => return Some(format_date(next_weekday(today, 5))),
        "sunday" | "sun" => return Some(format_date(next_weekday(today, 6))),
        "next-week" | "nextweek" => return Some(format_date(today + Duration::weeks(1))),
        "next-month" | "nextmonth" => return Some(format_date(add_months(today, 1))),
        "eow" | "end-of-week" => return Some(format_date(end_of_week(today))),
        "eom" | "end-of-month" => return Some(format_date(end_of_month(today))),
        _ => {}
    }

    // Handle relative dates: +3d, -2d, +1w, etc.
    if let Some(relative) = parse_relative_date(&input, today) {
        return Some(format_date(relative));
    }

    // Handle ISO date format (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
        return Some(format_date(date));
    }

    // Handle MM/DD/YYYY or DD/MM/YYYY (US format assumed)
    if let Ok(date) = NaiveDate::parse_from_str(&input, "%m/%d/%Y") {
        return Some(format_date(date));
    }

    // Handle MM-DD-YYYY
    if let Ok(date) = NaiveDate::parse_from_str(&input, "%m-%d-%Y") {
        return Some(format_date(date));
    }

    // If it's already a valid date format, pass through
    // This handles cases where the user provides a full ISO date
    if input.len() == 10 && input.contains('-') {
        return Some(input);
    }

    None
}

fn parse_relative_date(input: &str, today: NaiveDate) -> Option<NaiveDate> {
    let chars: Vec<char> = input.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let (sign, rest) = if chars[0] == '+' {
        (1i64, &input[1..])
    } else if chars[0] == '-' {
        (-1i64, &input[1..])
    } else {
        return None;
    };

    // Parse number and unit
    let unit = rest.chars().last()?;
    let num_str = &rest[..rest.len() - 1];
    let num: i64 = num_str.parse().ok()?;

    match unit {
        'd' => Some(today + Duration::days(sign * num)),
        'w' => Some(today + Duration::weeks(sign * num)),
        'm' => Some(add_months(today, (sign * num) as i32)),
        _ => None,
    }
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn next_weekday(from: NaiveDate, target_weekday: u32) -> NaiveDate {
    let current = from.weekday().num_days_from_monday();
    let days_until = if target_weekday > current {
        target_weekday - current
    } else {
        7 - current + target_weekday
    };
    from + Duration::days(days_until as i64)
}

fn end_of_week(from: NaiveDate) -> NaiveDate {
    let current = from.weekday().num_days_from_monday();
    let days_until_sunday = 6 - current; // Sunday is 6
    from + Duration::days(days_until_sunday as i64)
}

fn end_of_month(from: NaiveDate) -> NaiveDate {
    let year = from.year();
    let month = from.month();

    // Get the first day of next month, then subtract 1 day
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };

    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .map(|d| d - Duration::days(1))
        .unwrap_or(from)
}

fn add_months(from: NaiveDate, months: i32) -> NaiveDate {
    let total_months = from.year() * 12 + from.month() as i32 + months;
    let new_year = (total_months - 1) / 12;
    let new_month = ((total_months - 1) % 12 + 1) as u32;

    NaiveDate::from_ymd_opt(new_year, new_month, from.day().min(28))
        .unwrap_or(from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_today() {
        let result = parse_due_date("today");
        let expected = Local::now().date_naive().format("%Y-%m-%d").to_string();
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_tomorrow() {
        let result = parse_due_date("tomorrow");
        let expected = (Local::now().date_naive() + Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_relative_days() {
        let today = Local::now().date_naive();
        let result = parse_due_date("+3d");
        let expected = (today + Duration::days(3)).format("%Y-%m-%d").to_string();
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_relative_weeks() {
        let today = Local::now().date_naive();
        let result = parse_due_date("+2w");
        let expected = (today + Duration::weeks(2)).format("%Y-%m-%d").to_string();
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_iso_date() {
        let result = parse_due_date("2024-03-15");
        assert_eq!(result, Some("2024-03-15".to_string()));
    }
}
