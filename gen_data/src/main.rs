use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use anisearch::{AnisearchClient, DubStatus, DubbedAnime};
use clap::Parser;
use database::Root;

mod anisearch;
mod cli;
mod database;
mod logger;
mod output;

fn main() {
    // Parse arguments
    let args = cli::Args::parse();

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
    let mut dub_never_released_mal_ids: HashSet<u64> = HashSet::new();

    let anisearch_client = AnisearchClient::new(&args.language);

    log::info!("Checking dubbed anime page 1/??...");
    let page1_results = anisearch_client.get_dubbed_anime_list(1).unwrap();
    process_dubbed_page(&mut dubbed_mal_ids, &mut anisearch_map, &page1_results);
    dubbed_anisearch_urls.extend(page1_results.anisearch_urls.into_vec());

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

        let page_x_results = anisearch_client.get_dubbed_anime_list(page).unwrap();
        process_dubbed_page(&mut dubbed_mal_ids, &mut anisearch_map, &page_x_results);
        dubbed_anisearch_urls.extend(page_x_results.anisearch_urls.into_vec().into_iter());

        progress_bar.inc(1);
        std::thread::sleep(Duration::from_secs(1));
    }

    // Save dubbed MyAnimeList ids as temporary result
    let mut sorted_dubbed_mal_ids: Vec<u64> = dubbed_mal_ids.into_iter().collect();
    sorted_dubbed_mal_ids.sort_unstable();

    output::write_output(output_path, &sorted_dubbed_mal_ids, &[]);

    // Check for incomplete dubs
    progress_bar.set_position(0);
    progress_bar.set_length(dubbed_anisearch_urls.len() as u64);

    for (index, dubbed_anisearch_url) in dubbed_anisearch_urls.iter().enumerate() {
        log::info!(
            "Checking if dub is complete {}/{}: {}",
            index + 1,
            dubbed_anisearch_urls.len(),
            dubbed_anisearch_url
        );

        let mut add_to_incomplete_mal_ids = || {
            if let Some(anime_entry_refcell) = anisearch_map.get(dubbed_anisearch_url.deref()) {
                let mal_ids = &anime_entry_refcell.borrow().mal_ids;
                dub_incomplete_mal_ids.extend(mal_ids.iter());
            }
        };

        match anisearch_client.get_dub_status(dubbed_anisearch_url) {
            Ok(DubStatus::Complete) => {}
            Ok(DubStatus::Incomplete | DubStatus::Upcoming) => {
                // For now, I treat upcoming anime as incomplete
                add_to_incomplete_mal_ids();
                log::info!("Dub is incomplete: {}", dubbed_anisearch_url);
            }
            Ok(DubStatus::NeverReleased) => {
                if let Some(anime_entry_refcell) = anisearch_map.get(dubbed_anisearch_url.deref()) {
                    let mal_ids = &anime_entry_refcell.borrow().mal_ids;
                    dub_never_released_mal_ids.extend(mal_ids.iter());
                    log::info!("Dub has never been released: {}", dubbed_anisearch_url);
                }
            }
            Err(_) => {
                // I prefer to treat it as incomplete, if it cannot verify the completeness
                // Happens with: https://anisearch.com/anime/18285
                add_to_incomplete_mal_ids();
                log::error!("Failed to check if the dub is complete for: {}", dubbed_anisearch_url);
            }
        };

        progress_bar.inc(1);
        std::thread::sleep(Duration::from_secs(1));
    }

    // Remove never released dubs
    for dub_never_released_mal_id in dub_never_released_mal_ids {
        dub_incomplete_mal_ids.remove(&dub_never_released_mal_id);
        sorted_dubbed_mal_ids.retain(|&mal_id| mal_id != dub_never_released_mal_id);
    }

    // Save dubbed MyAnimeList ids, with incomplete information
    let mut sorted_dub_incomplete_mal_ids: Vec<u64> = dub_incomplete_mal_ids.into_iter().collect();
    sorted_dub_incomplete_mal_ids.sort_unstable();

    output::write_output(output_path, &sorted_dubbed_mal_ids, &sorted_dub_incomplete_mal_ids);

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
        let Some(anime_entry_refcell) = anisearch_map.get_mut(anisearch_url.deref()) else {
            continue;
        };
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

#[cfg(test)]
mod tests {
    use super::mal_parse_id;

    #[test]
    fn test_parse_mal_id() {
        assert_eq!(mal_parse_id("https://myanimelist.net/anime/1535"), Some(1535));
    }
}
