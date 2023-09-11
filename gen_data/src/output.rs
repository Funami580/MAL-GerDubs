use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;

use serde::Serialize;

#[derive(Serialize)]
struct Output<'a> {
    dubbed: &'a [u64],
    incomplete: &'a [u64],
}

pub fn write_output(path: &Path, dubbed_mal_ids: &[u64], incomplete_mal_ids: &[u64]) {
    std::fs::create_dir_all(path.parent().unwrap()).ok();

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .expect("failed to open output file");
    let mut writer = BufWriter::new(file);

    let output = Output {
        dubbed: dubbed_mal_ids,
        incomplete: incomplete_mal_ids,
    };
    serde_json::to_writer_pretty(&mut writer, &output).expect("failed to write to output");

    writer.flush().expect("failed to flush output");
}
