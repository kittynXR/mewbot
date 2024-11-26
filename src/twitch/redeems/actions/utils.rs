pub fn split_response(response: String) -> (String, Option<String>) {
    if response.len() <= 495 {
        (response, None)
    } else {
        // Replace line breaks with a special marker
        let processed = response.replace('\n', " ↵ ");
        let first_part = processed.chars().take(490).collect::<String>();

        // Try to split at the last sentence end before 500 chars
        let valid_endings = ['.', '!', '?'];
        let split_pos = valid_endings.iter()
            .filter_map(|&end| first_part.rfind(end))
            .max()
            .unwrap_or(497);

        let (first, second) = processed.split_at(split_pos + 1);
        let first_part = first.trim().to_string();
        let second_part = second.trim().to_string();

        if second_part.is_empty() {
            (first_part, None)
        } else {
            (format!("{}...", first_part), Some(second_part))
        }
    }
}