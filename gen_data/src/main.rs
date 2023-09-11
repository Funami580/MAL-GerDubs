use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use database::Root;
use reqwest::StatusCode;

mod database;
mod logger;
mod output;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0";

fn main() {
    // Set up logger
    let logger = logger::default_logger();
    let multi = indicatif::MultiProgress::new();

    indicatif_log_bridge::LogWrapper::new(multi.clone(), logger)
        .try_init()
        .unwrap();

    // Read database
    let db_path = Path::new("../anime-offline-database/anime-offline-database-minified.json");
    let output_path = Path::new("../data/dubInfo.json");
    let root = database::read_database(db_path);
    assert!(root.data.len() > 0);

    // Process...
    let mut anisearch_map = get_anisearch_map(&root);
    let mut dubbed_mal_ids: HashSet<u64> = HashSet::new();
    let mut dubbed_anisearch_urls: HashSet<String> = HashSet::new();
    let mut dub_incomplete_mal_ids: HashSet<u64> = HashSet::new();

    let client = get_default_client();
    let dubbed_anime_fetcher = get_dubbed_anime_fetcher(&client);

    log::info!("Checking dubbed anime page 1/??...");
    let page1_results = dubbed_anime_fetcher(&get_dubbed_anime_page_url(1)).unwrap();
    process_dubbed_page(&mut dubbed_mal_ids, &mut anisearch_map, &page1_results);
    dubbed_anisearch_urls.extend(page1_results.anisearch_urls.into_vec().into_iter());

    let progress_bar = {
        let pb = indicatif::ProgressBar::new(page1_results.total_pages);
        pb.set_style(
            indicatif::ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
        );
        multi.add(pb)
    };

    progress_bar.inc(1);

    for page in 2..=page1_results.total_pages {
        log::info!("Checking dubbed anime page {}/{}...", page, page1_results.total_pages);
        let page_x_results = dubbed_anime_fetcher(&get_dubbed_anime_page_url(page)).unwrap();
        process_dubbed_page(&mut dubbed_mal_ids, &mut anisearch_map, &page_x_results);
        dubbed_anisearch_urls.extend(page_x_results.anisearch_urls.into_vec().into_iter());
        progress_bar.inc(1);
        std::thread::sleep(Duration::from_secs(1));
    }

    // Save dubbed MyAnimeList ids as temporary result
    let mut sorted_dubbed_ids: Vec<u64> = dubbed_mal_ids.into_iter().collect();
    sorted_dubbed_ids.sort_unstable();

    output::write_output(output_path, &sorted_dubbed_ids, &[]);

    // Check for incomplete dubs
    let dub_complete_checker = get_dub_complete_checker(&client);
    progress_bar.set_position(0);
    progress_bar.set_length(dubbed_anisearch_urls.len() as u64);

    for (index, dubbed_anisearch_url) in dubbed_anisearch_urls.iter().enumerate() {
        log::info!(
            "Checking if dub is complete {}/{}: {}",
            index + 1,
            dubbed_anisearch_urls.len(),
            dubbed_anisearch_url
        );

        match dub_complete_checker(dubbed_anisearch_url) {
            Ok(true) => {}
            Ok(false) => {
                if let Some(anime_entry_refcell) = anisearch_map.get(dubbed_anisearch_url.deref()) {
                    let mal_ids = &anime_entry_refcell.borrow().mal_ids;
                    dub_incomplete_mal_ids.extend(mal_ids.iter());
                    log::info!("Dub is incomplete: {}", dubbed_anisearch_url);
                }
            }
            Err(_) => {
                // I prefer to treat it as incomplete, if it cannot verify the completeness
                // Happens with: https://www.anisearch.com/anime/18285
                if let Some(anime_entry_refcell) = anisearch_map.get(dubbed_anisearch_url.deref()) {
                    let mal_ids = &anime_entry_refcell.borrow().mal_ids;
                    dub_incomplete_mal_ids.extend(mal_ids.iter());
                    log::error!("Failed to check if the dub is complete for: {}", dubbed_anisearch_url);
                }
            }
        };

        progress_bar.inc(1);
        std::thread::sleep(Duration::from_secs(1));
    }

    // Save dubbed MyAnimeList ids, with incomplete information
    let mut sorted_incomplete_ids: Vec<u64> = dub_incomplete_mal_ids.into_iter().collect();
    sorted_incomplete_ids.sort_unstable();

    output::write_output(output_path, &sorted_dubbed_ids, &sorted_incomplete_ids);

    // Clean up
    progress_bar.finish();
    multi.remove(&progress_bar);
}

fn process_dubbed_page(
    dubbed_mal_ids: &mut HashSet<u64>,
    anisearch_map: &mut HashMap<&str, Rc<RefCell<AnimeEntry>>>,
    dubbed_anime: &DubbedAnime,
) {
    for anisearch_url in dubbed_anime.anisearch_urls.iter() {
        let Some(anime_entry_refcell) = anisearch_map.get_mut(anisearch_url.deref()) else { continue; };
        let mut anime_entry = anime_entry_refcell.borrow_mut();

        anime_entry.current_validations += 1;

        if anime_entry.current_validations == anime_entry.validations_required {
            dubbed_mal_ids.extend(anime_entry.mal_ids.iter());
        }
    }
}

struct AnimeEntry {
    mal_ids: Box<[u64]>,
    validations_required: u64,
    current_validations: u64,
}

fn get_anisearch_map<'a>(root: &'a Root) -> HashMap<&'a str, Rc<RefCell<AnimeEntry>>> {
    let mut anisearch_map: HashMap<&'a str, Rc<RefCell<AnimeEntry>>> = HashMap::with_capacity(root.data.len());

    for anime in root.data.iter() {
        let mal_urls: Box<[&str]> = anime
            .sources
            .iter()
            .filter(|&src| src.starts_with("https://myanimelist.net/"))
            .map(|src| src.deref())
            .collect();

        if mal_urls.is_empty() {
            continue;
        }

        let mal_ids: Box<[u64]> = mal_urls
            .iter()
            .filter_map(|&mal_url| {
                let id = mal_parse_id(mal_url);

                if id.is_none() {
                    log::warn!("Failed to parse id from MyAnimeList URL: {}", mal_url);
                }

                id
            })
            .collect();

        if mal_urls.len() != mal_ids.len() {
            continue;
        }

        let anisearch_urls: Box<[&str]> = anime
            .sources
            .iter()
            .filter(|&src| src.starts_with("https://anisearch.com/"))
            .map(|src| src.deref())
            .collect();

        let anime_entry = Rc::new(RefCell::new(AnimeEntry {
            mal_ids,
            validations_required: anisearch_urls.len() as u64,
            current_validations: 0,
        }));

        for anisearch_url in anisearch_urls.iter() {
            anisearch_map.insert(anisearch_url, anime_entry.clone());
        }
    }

    anisearch_map
}

fn mal_parse_id(anime_url: &str) -> Option<u64> {
    anime_url
        .strip_prefix("https://myanimelist.net/anime/")
        .and_then(|id| id.parse().ok())
}

struct DubbedAnime {
    total_pages: u64,
    anisearch_urls: Box<[String]>,
}

fn get_dubbed_anime_page_url(page: u64) -> String {
    format!("https://www.anisearch.com/anime/index/page-{page}?synchro=de&sort=title&order=asc&view=2&limit=100")
}

fn get_dubbed_anime_fetcher(
    client: &reqwest::blocking::Client,
) -> impl for<'b> Fn(&'b str) -> Result<DubbedAnime, ()> + '_ {
    let a_selector = scraper::Selector::parse(r#"th > a[lang]"#).unwrap();
    let page_info_selector = scraper::Selector::parse(r#"div.pagenav-info"#).unwrap();

    move |anisearch_url| {
        let document = get_anisearch_page(client, anisearch_url)?;
        let total_pages = document
            .select(&page_info_selector)
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
            .select(&a_selector)
            .filter_map(|a_element| {
                let href = match a_element.value().attr("href") {
                    Some(link) => link,
                    None => {
                        log::error!("Got <a> element without href for: {}", anisearch_url);
                        return None;
                    }
                };

                format_anisearch_link(href).ok()
            })
            .collect();

        Ok(DubbedAnime {
            total_pages,
            anisearch_urls: dubbed_elements,
        })
    }
}

fn get_anisearch_page(client: &reqwest::blocking::Client, anisearch_url: &str) -> Result<scraper::Html, ()> {
    let mut too_many_requests: u64 = 0;

    let body = loop {
        let response = client.get(anisearch_url).send();
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
                    too_many_requests += 1;
                    wait_request_failed("Too many requests", 60 * too_many_requests);
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
                wait_request_failed("Request failed", 10);
                continue;
            }
        };

        break body;
    };

    Ok(scraper::Html::parse_document(&body))
}

fn wait_request_failed(message: &str, seconds: u64) {
    for second in (1..=seconds).rev() {
        log::info!("{message}, retrying in {second}...");
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn format_anisearch_link(url: &str) -> Result<String, ()> {
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

fn get_dub_complete_checker(client: &reqwest::blocking::Client) -> impl for<'b> Fn(&'b str) -> Result<bool, ()> + '_ {
    let status_selector = scraper::Selector::parse(r#"div.title[lang="de"] + div.status"#).unwrap();

    move |anisearch_url| {
        let document = get_anisearch_page(client, anisearch_url)?;

        Ok(document
            .select(&status_selector)
            .next()
            .ok_or(())?
            .text()
            .collect::<String>()
            .to_ascii_lowercase()
            .contains("completed"))
    }
}

fn get_default_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(20))
        .build()
        .unwrap()
}

#[test]
fn test_format_anisearch_link() {
    assert_eq!(
        format_anisearch_link("anime/1540,alps-monogatari-watashi-no-annette"),
        Ok("https://anisearch.com/anime/1540".to_string())
    );
}

#[test]
fn test_parse_mal_id() {
    assert_eq!(mal_parse_id("https://myanimelist.net/anime/1535"), Some(1535));
}

#[test]
fn test_is_dub_complete() {
    let client = get_default_client();
    let dub_complete_checker = get_dub_complete_checker(&client);

    assert!(dub_complete_checker("https://anisearch.com/anime/15141").unwrap());
    assert!(!dub_complete_checker("https://anisearch.de/anime/14").unwrap());
}
