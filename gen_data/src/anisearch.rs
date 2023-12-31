use std::time::Duration;

use reqwest::StatusCode;
use scraper::Selector;

use crate::cli::Language;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0";

pub struct AnisearchClient<'a> {
    client: reqwest::blocking::Client,
    lang: &'a str,
    selector_dubbed_anime_list_page_info: Selector,
    selector_dubbed_anime_list_anime_url: Selector,
    selector_anime_dub_info: Selector,
    selector_anime_dub_status: Selector,
}

pub struct DubbedAnime {
    pub total_pages: u64,
    pub anisearch_urls: Box<[String]>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DubStatus {
    Complete,
    Incomplete,
    Upcoming,
    NeverReleased,
}

impl AnisearchClient<'_> {
    pub fn new(language: &Language) -> Self {
        let anisearch_lang = language.get_anisearch_language();
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(20))
            .build()
            .unwrap();

        Self {
            client,
            lang: anisearch_lang,
            selector_dubbed_anime_list_page_info: scraper::Selector::parse(r#"div.pagenav-info"#).unwrap(),
            selector_dubbed_anime_list_anime_url: scraper::Selector::parse(r#"th > a[lang]"#).unwrap(),
            selector_anime_dub_info: scraper::Selector::parse(&format!(r#"div.title[lang="{anisearch_lang}"]"#))
                .unwrap(),
            selector_anime_dub_status: scraper::Selector::parse(&format!(
                r#"div.title[lang="{anisearch_lang}"] + div.status"#
            ))
            .unwrap(),
        }
    }
}

impl AnisearchClient<'_> {
    fn get_page(&self, anisearch_url: &str) -> Result<scraper::Html, ()> {
        fn wait_request_failed(message: &str, seconds: u64) {
            for second in (1..=seconds).rev() {
                log::info!("{message}, retrying in {second}...");
                std::thread::sleep(Duration::from_secs(1));
            }
        }

        const MAX_BACKOFF_SECONDS: u64 = 300; // max. 5 min backoff
        let mut backoff_count: u64 = 0;

        let body = loop {
            let response = self.client.get(anisearch_url).send();
            let body = match response {
                Ok(res) => match res.status() {
                    StatusCode::OK => match res.text() {
                        Ok(text) => text,
                        Err(err) => {
                            log::error!("Failed to parse text for: {}. Error: {}", anisearch_url, err);
                            return Err(());
                        }
                    },
                    StatusCode::TOO_MANY_REQUESTS => {
                        backoff_count += 1;
                        wait_request_failed("Too many requests", (60 * backoff_count).min(MAX_BACKOFF_SECONDS));
                        continue;
                    }
                    err if err.is_server_error() => {
                        log::error!("aniSearch returned server error for: {}", anisearch_url);
                        return Err(());
                    }
                    err => {
                        log::error!("aniSearch returned error for: {}. Error: {}", anisearch_url, err);
                        return Err(());
                    }
                },
                Err(_) => {
                    wait_request_failed("Request failed", (10 * backoff_count).min(MAX_BACKOFF_SECONDS));
                    continue;
                }
            };

            break body;
        };

        Ok(scraper::Html::parse_document(&body))
    }

    pub fn get_dubbed_anime_list(&self, page: u64) -> Result<DubbedAnime, ()> {
        let lang = self.lang;
        let url = format!(
            "https://www.anisearch.com/anime/index/page-{page}?synchro={lang}&sort=title&order=asc&view=2&limit=100"
        );
        let document = self.get_page(&url)?;
        let total_pages = document
            .select(&self.selector_dubbed_anime_list_page_info)
            .next()
            .unwrap()
            .text()
            .collect::<String>()
            .trim()
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>()
            .parse::<u64>()
            .unwrap();
        let dubbed_elements = document
            .select(&self.selector_dubbed_anime_list_anime_url)
            .filter_map(|a_element| {
                let href = match a_element.value().attr("href") {
                    Some(link) => link,
                    None => {
                        log::error!("Got <a> element without href for: {}", url);
                        return None;
                    }
                };

                Self::format_anisearch_url(href).ok()
            })
            .collect();

        Ok(DubbedAnime {
            total_pages,
            anisearch_urls: dubbed_elements,
        })
    }

    pub fn get_dub_status(&self, anime_url: &str) -> Result<DubStatus, ()> {
        let document = self.get_page(anime_url)?;
        let status_text = document
            .select(&self.selector_anime_dub_status)
            .next()
            .ok_or(())?
            .text()
            .collect::<String>()
            .to_ascii_lowercase();

        Ok(if status_text.contains("completed") {
            DubStatus::Complete
        } else if status_text.contains("upcoming") {
            DubStatus::Upcoming
        } else {
            let never_released = document
                .select(&self.selector_anime_dub_info)
                .next()
                .ok_or(())?
                .text()
                .collect::<String>()
                .to_ascii_lowercase()
                .contains("never released");

            if never_released {
                DubStatus::NeverReleased
            } else {
                DubStatus::Incomplete
            }
        })
    }

    fn format_anisearch_url(url: &str) -> Result<String, ()> {
        // anime/1540,alps-monogatari-watashi-no-annette
        // -> https://anisearch.com/anime/1540
        let url = url.to_lowercase();
        let url = if let Some(stripped) = url.strip_prefix("https://www.") {
            format!("https://{}", stripped)
        } else if url.starts_with("https://") {
            url.to_string()
        } else if let Some(stripped) = url.strip_prefix("www.") {
            format!("https://{}", stripped)
        } else if url.starts_with("anisearch.com/") {
            format!("https://{}", url)
        } else {
            format!("https://anisearch.com/{}", url)
        };

        let anime_prefix = "https://anisearch.com/anime/";

        if let Some(id_and_name) = url.strip_prefix(anime_prefix) {
            let id = id_and_name
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>();
            Ok(format!("{}{}", anime_prefix, id))
        } else {
            log::error!("Could not format aniSearch url: {}", url);
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::AnisearchClient;
    use crate::{anisearch::DubStatus, cli::Language};

    #[test]
    fn test_format_anisearch_url() {
        assert_eq!(
            AnisearchClient::format_anisearch_url("anime/1540,alps-monogatari-watashi-no-annette"),
            Ok("https://anisearch.com/anime/1540".to_string())
        );
    }

    #[test]
    fn test_get_dub_status() {
        let anisearch_client = AnisearchClient::new(&Language::German);

        assert_eq!(
            anisearch_client.get_dub_status("https://anisearch.com/anime/15141"),
            Ok(DubStatus::Complete)
        );
        assert_eq!(
            anisearch_client.get_dub_status("https://anisearch.com/anime/14"),
            Ok(DubStatus::Incomplete)
        );
        assert_eq!(
            anisearch_client.get_dub_status("https://anisearch.com/anime/2852"),
            Ok(DubStatus::NeverReleased)
        );
        assert!(matches!(
            anisearch_client.get_dub_status("https://anisearch.com/anime/18285"),
            Err(_)
        ));
    }

    #[test]
    fn test_never_released() {
        let never_released_anisearch_ids = [
            4105, 2507, 1152, 2575, 5654, 2652, 3417, 2949, 2375, 329, 1467, 5734, 2626, 436, 409, 89, 2852, 3867,
            4004, 3735, 4421, 6655, 6671, 7268, 10083, 11787, 12916, 14791,
        ];

        let anisearch_client = AnisearchClient::new(&Language::German);

        for id in never_released_anisearch_ids {
            assert_eq!(
                anisearch_client.get_dub_status(&format!("https://anisearch.com/anime/{id}")),
                Ok(DubStatus::NeverReleased),
                "failed for id {id}"
            );
            std::thread::sleep(Duration::from_millis(500));
        }
    }
}
