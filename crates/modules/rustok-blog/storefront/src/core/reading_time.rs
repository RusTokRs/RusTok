pub fn calculate_reading_time(plain_text: &str) -> u32 {
    let word_count = plain_text.split_whitespace().count();
    let minutes = word_count.div_ceil(200);
    (minutes as u32).max(1)
}

pub fn format_reading_time(minutes: u32, locale: Option<&str>) -> String {
    if locale == Some("ru") {
        let mod10 = minutes % 10;
        let mod100 = minutes % 100;
        let suffix = if mod10 == 1 && mod100 != 11 {
            "минута"
        } else if (2..=4).contains(&mod10) && !(10..=20).contains(&mod100) {
            "минуты"
        } else {
            "минут"
        };
        format!("{minutes} {suffix} чтения")
    } else {
        format!("{minutes} min read")
    }
}
