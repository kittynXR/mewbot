pub fn split_response(response: String) -> (String, Option<String>) {
    if response.len() <= 500 {
        (response, None)
    } else {
        // Try to split at the last complete sentence before 500 chars
        let valid_endings = ['.', '!', '?'];
        let first_part = response.chars().take(497).collect::<String>();

        let split_pos = valid_endings.iter()
            .filter_map(|&end| first_part.rfind(end))
            .max()
            .unwrap_or(497);

        let (first, second) = response.split_at(split_pos + 1);
        let first_part = first.trim().to_string();
        let second_part = second.trim().to_string();

        if second_part.is_empty() {
            (first_part, None)
        } else {
            (format!("{}...", first_part), Some(second_part))
        }
    }
}