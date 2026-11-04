use super::routing::percent_encode;

pub fn telegram_share_url(url: &str, text: &str) -> String {
    format!(
        "https://t.me/share/url?url={}&text={}",
        percent_encode(url),
        percent_encode(text)
    )
}

pub fn vk_share_url(url: &str, title: &str) -> String {
    format!(
        "https://vk.com/share.php?url={}&title={}",
        percent_encode(url),
        percent_encode(title)
    )
}

pub fn twitter_share_url(url: &str, text: &str) -> String {
    format!(
        "https://twitter.com/intent/tweet?url={}&text={}",
        percent_encode(url),
        percent_encode(text)
    )
}
