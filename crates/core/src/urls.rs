use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

pub fn apple_music_search_url(artist: &str, title: &str) -> String {
    let q = format!("{} {}", artist, title);
    let encoded = utf8_percent_encode(&q, NON_ALPHANUMERIC).to_string();
    format!("https://music.apple.com/us/search?term={encoded}")
}

pub fn spotify_search_url(artist: &str, title: &str) -> String {
    let q = format!("{} {}", artist, title);
    let encoded = utf8_percent_encode(&q, NON_ALPHANUMERIC).to_string();
    format!("https://open.spotify.com/search/{encoded}")
}

#[cfg(test)]
mod tests {
    use super::{apple_music_search_url, spotify_search_url};

    #[test]
    fn url_builder_encodes_queries() {
        let apple = apple_music_search_url("Daft Punk", "Get Lucky");
        let spotify = spotify_search_url("AC/DC", "Back In Black");

        assert!(apple.contains("Daft%20Punk%20Get%20Lucky"));
        assert!(spotify.contains("AC%2FDC%20Back%20In%20Black"));
    }
}
