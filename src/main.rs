//! # Text_Analysis
//! Analyze text stored as *.txt or *pdf in chosen directory. Doesn't read files in subdirectories.
//! Counting all words and then searching for every unique word in the vicinity (+-5 words).
//! Stores results in file [date/time]results_word_analysis.txt
//! ## Usage: ```text_analysis path```

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::fs::{read_dir, File};
use std::io::prelude::*;
use std::panic;
use std::path::PathBuf;
use std::sync::mpsc::sync_channel;
use std::thread::spawn;
use std::time::Instant;

use chrono::prelude::*;
use pdf_extract::*;
use rayon::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let path = PathBuf::from(&args[1]);
    let instant = Instant::now();
    //let current_dir = env::current_dir()?;
    let mut documents = Vec::new();
    // for entry in read_dir(path).unwrap() { //is directory of executable should be analyzed
    for entry in read_dir(&path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file()
            && !path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("results_word_analysis")
            && path.extension().and_then(OsStr::to_str) == Some("txt")
            || path.extension().and_then(OsStr::to_str) == Some("pdf")
        {
            documents.push(path);
        }
    }

    let mut content_string_all = String::new();

    let (sender, receiver) = sync_channel(32);

    spawn(move || {
        for filename in documents {
            let sender = sender.clone();
            spawn(move || {
                if filename.extension().and_then(OsStr::to_str) == Some("txt") {
                    let mut f: File = File::open(filename).unwrap();
                    let mut text = String::new();
                    f.read_to_string(&mut text).unwrap();
                    if sender.send(text).is_err() {
                        //break;
                    }
                } else if filename.extension().and_then(OsStr::to_str) == Some("pdf") {
                    let text: String = panic::catch_unwind(|| {
                        extract_text(filename).unwrap_or_else(|_| "rust_error".to_string())
                    })
                    .unwrap();
                    if sender.send(text).is_err() {
                        //break;
                    }
                }
            });
        }
    });

    for text in receiver {
        content_string_all.push_str(&text);
    }

    let content_vec: Vec<String> = trim_to_words(content_string_all)?;

    println!("Total number of words read: {:?}", content_vec.len());

    let word_frequency = count_words(&content_vec)?;
    let words_sorted = sort_map_to_vec(word_frequency)?;

    let words_len = words_sorted.len();

    println!(
        "Counted words in {:?}. Number of unique words: {:?} \n Finding words near:",
        instant.elapsed(),
        words_len
    );

    let mut index_rang: usize = 0;
    let mut words_near_map: HashMap<String, HashMap<String, u32>> = HashMap::new();
    for word in &words_sorted {
        println!(
            "Analyzing nearest words for word n° {:?} of {:?}",
            index_rang + 1,
            &words_len
        );
        words_near_map.extend(words_near(&word, index_rang, &content_vec, &words_sorted)?);

        index_rang += 1;
    }
    //println!("Words: {:?}", words_sorted);
    //println!("Words near: {:?}", words_near_map);

    println!(
        "Finished analyzing words in {:?}.\nPreparing output:",
        instant.elapsed()
    );

    let mut to_file = String::new();

    let mut i = 1 as usize;
    for word in words_sorted {
        println!("Formatting word-analysis n° {:?} of {:?}", i, &words_len);
        let (word_only, frequency) = &word;
        let words_near = &words_near_map[word_only];
        let combined = format!(
            "Word: {:?}, Frequency: {:?},\nWords near: {:?} \n\n",
            word_only,
            frequency,
            sort_map_to_vec(words_near.to_owned())?
        );
        to_file.push_str(&combined);
        i += 1;
    }

    save_file(to_file, path)?;

    println!(
        "Finished in {:?}! Please see file for results",
        instant.elapsed()
    );

    Ok(())
}

//search for words +-5 around given word. Returns result.
fn words_near(
    word: &(String, u32),
    index_rang: usize,
    content_vec: &Vec<String>,
    words_sorted: &Vec<(String, u32)>,
) -> std::io::Result<HashMap<String, HashMap<String, u32>>> {
    let index: Vec<usize> = positions(&content_vec, &words_sorted[index_rang].0);
    let mut vec_word = Vec::new();
    for index_single in index {
        let mut count = 1 as usize;
        for i in 0..content_vec.len() {
            let min: usize = get_index_min(&index_single)?;
            let max: usize = get_index_max(&index_single)?;
            if i >= min && count <= max && i != index_single {
                vec_word.push(content_vec[i].clone());
                count += 1;
            } else {
            };
        }
    }
    let words_near: Vec<(String, u32)> = sort_map_to_vec(count_words(&vec_word)?)?;
    let mut words_near_map: HashMap<String, HashMap<String, u32>> = HashMap::new();
    words_near_map.insert(word.0.to_owned(), words_near.into_par_iter().collect());
    //println!("insert word: {:?}, map: {:?}", word, words_near);
    Ok(words_near_map)
}

//search for all position (usize) of word in given Vector<String>
fn positions(vector: &Vec<String>, target: &String) -> Vec<usize> {
    let mut res = Vec::new();
    for (index, c) in vector.into_iter().enumerate() {
        if &c == &target {
            res.push(index)
        }
    }
    res
}

//count included in given &Vec<String>. Returns result as HashMap. Returns result.
fn count_words(words: &Vec<String>) -> std::io::Result<HashMap<String, u32>> {
    let mut frequency: HashMap<String, u32> = HashMap::new();
    for word in words {
        //ignore words constiting of only one char
        if word.len() > 1 {
            *frequency.entry(word.to_owned()).or_insert(0) += 1;
        }
    }
    Ok(frequency)
}

//sort words in HashMap<Word, Frequency> according to frequency  into Vector. Returns result.
fn sort_map_to_vec(frequency: HashMap<String, u32>) -> std::io::Result<Vec<(String, u32)>> {
    let mut vec_sorted: Vec<(String, u32)> = frequency.into_par_iter().collect();
    vec_sorted.par_sort_by(|a, b| b.1.cmp(&a.1));
    Ok(vec_sorted)
}

//splits content of file into singe words as Vector<String>. Returns result.
fn trim_to_words(content: String) -> std::io::Result<Vec<String>> {
    let content: Vec<String> = content
        .to_lowercase()
        .replace(&['-'][..], " ")
        .replace(
            &[
                '(', ')', ',', '\"', '.', ';', ':', '=', '[', ']', '{', '}', '-', '_', '/', '\'',
                '’', '?', '!', '“', '‘',
            ][..],
            "",
        )
        .split_whitespace()
        .map(String::from)
        .collect::<Vec<String>>();
    Ok(content)
}

//get mininum index.
fn get_index_min(index: &usize) -> std::io::Result<usize> {
    let min = if *index == 4 {
        index - 4
    } else if *index == 3 {
        index - 3
    } else if *index == 2 {
        index - 2
    } else if *index == 1 {
        index - 1
    } else if *index == 0 {
        0
    } else {
        index - 5
    };
    Ok(min)
}

//get maximum index.
fn get_index_max(index: &usize) -> std::io::Result<usize> {
    let max = if *index == 4 {
        9
    } else if *index == 3 {
        8
    } else if *index == 2 {
        7
    } else if *index == 1 {
        6
    } else if *index == 0 {
        5
    } else {
        10
    };
    Ok(max)
}

//save file to path. Return result.
fn save_file(to_file: String, mut path: PathBuf) -> std::io::Result<()> {
    let local: DateTime<Local> = Local::now();
    let new_filename: String = local
        .format("%Y_%m_%d_%H_%M_%S_results_word_analysis.txt")
        .to_string();
    path.push(new_filename);

    let mut file = OpenOptions::new().write(true).create(true).open(path)?;

    file.write_all(to_file.as_bytes())?;

    Ok(())
}
