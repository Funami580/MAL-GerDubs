#![allow(dead_code)]
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::Deserialize;

pub type Url = String;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub license: License,
    pub repository: Url,
    pub last_update: String, // TODO
    pub data: Box<[Anime]>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anime {
    pub sources: Box<[Url]>,
    pub title: String,
    pub r#type: Type,
    pub episodes: u32,
    pub status: Status,
    pub anime_season: AnimeSeason,
    pub picture: Url,
    pub thumbnail: Url,
    pub synonyms: Box<[String]>,
    pub relations: Box<[Url]>,
    pub tags: Box<[String]>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Type {
    Tv,
    Movie,
    Ova,
    Ona,
    Special,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    Finished,
    Ongoing,
    Upcoming,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimeSeason {
    pub season: Season,
    pub year: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Season {
    Spring,
    Summer,
    Fall,
    Winter,
    Undefined,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct License {
    pub name: String,
    pub url: Url,
}

pub fn read_database(path: &Path) -> Root {
    // open the file in read-only mode with buffer
    let file = File::open(path).expect("database could not be found or opened");
    let reader = BufReader::new(file);

    // read the JSON contents of the file as an instance of `Root`
    serde_json::from_reader(reader).expect("failed to parse database")
}
