use std::io::{self, Write};

pub fn prompt(label: &str) -> Result<String, String> {
    print!("{label}: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("cannot write prompt: {error}"))?;
    let mut value = String::new();
    io::stdin()
        .read_line(&mut value)
        .map_err(|error| format!("cannot read input: {error}"))?;
    Ok(value.trim().to_owned())
}

pub fn prompt_default(label: &str, default: &str) -> Result<String, String> {
    let value = prompt(&format!("{label} [{default}]"))?;
    Ok(if value.is_empty() {
        default.to_owned()
    } else {
        value
    })
}

pub fn prompt_required(label: &str, default: Option<&str>) -> Result<String, String> {
    let value = match default {
        Some(default) => prompt_default(label, default)?,
        None => prompt(label)?,
    };
    if value.trim().is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(value)
    }
}

pub fn prompt_choice(label: &str, choices: &[&str]) -> Result<usize, String> {
    let value = prompt(label)?;
    parse_choice(label, choices.len(), &value)
}

fn parse_choice(label: &str, choice_count: usize, value: &str) -> Result<usize, String> {
    let selected = value
        .parse::<usize>()
        .ok()
        .filter(|selected| (1..=choice_count).contains(selected))
        .ok_or_else(|| format!("{label} must be a number from 1 to {choice_count}"))?;
    Ok(selected - 1)
}

pub fn prompt_yes_no(label: &str, default: bool) -> Result<bool, String> {
    let default = if default { "yes" } else { "no" };
    parse_yes_no(label, &prompt_default(label, default)?)
}

pub fn prompt_positive_u64(label: &str, default: u64) -> Result<u64, String> {
    prompt_default(label, &default.to_string())?
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label} must be a positive integer"))
}

pub fn prompt_positive_usize(label: &str, default: usize) -> Result<usize, String> {
    prompt_default(label, &default.to_string())?
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label} must be a positive integer"))
}

fn parse_yes_no(label: &str, value: &str) -> Result<bool, String> {
    match value {
        "yes" | "y" => Ok(true),
        "no" | "n" => Ok(false),
        _ => Err(format!("{label} must be yes or no")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yes_no_values_are_case_sensitive_and_unambiguous() {
        assert_eq!(parse_yes_no("Cover service", "yes"), Ok(true));
        assert_eq!(parse_yes_no("Cover service", "n"), Ok(false));
        assert!(parse_yes_no("Cover service", "true").is_err());
    }

    #[test]
    fn numbered_choices_map_to_zero_based_actions() {
        assert_eq!(parse_choice("Certificate", 3, "1"), Ok(0));
        assert_eq!(parse_choice("Certificate", 3, "3"), Ok(2));
        assert!(parse_choice("Certificate", 3, "0").is_err());
        assert!(parse_choice("Certificate", 3, "4").is_err());
    }
}
