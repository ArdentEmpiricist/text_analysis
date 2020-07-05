//! # Text_Analysis
//! Analyze text stored as *.txt or *pdf in provided file or directory. Doesn't read files in subdirectories.
//! Counting all words and then searching for every unique word in the vicinity (+-5 words).
//! Stores results in file [date/time]results_word_analysis.txt in given directory.
//! ## Usage: ```text_analysis path/to/directory_or_file```

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{read_dir, File};
use std::io::prelude::*;
use std::panic;
use std::path::PathBuf;
use std::sync::mpsc::sync_channel;
use std::thread::spawn;
use std::time::Instant;

use pdf_extract::*;

use text_analysis::{count_words, save_file, sort_map_to_vec, trim_to_words, words_near};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let path = PathBuf::from(&args[1]);
    let instant = Instant::now();
    //Vec documents will contain filenames of readable files in directory
    let mut documents = Vec::new();
    //path_dir is the directory to save results file in.
    let mut path_dir: PathBuf = PathBuf::new();
    if path.is_file() {
        //Ckeck if argument is a file
        path_dir.push(path.clone().parent().unwrap());
        documents.push(path.clone())
    } else if path.is_dir() {
        //Ckeck if argument is a directory
        path_dir.push(path.clone());
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
    } else {
        panic!("Provided argument is neither directory nor file. Please check.")
    }

    //create String to hold all text
    let mut content_string_all = String::new();

    //create channels to receive read String from files
    let (sender, receiver) = sync_channel(32);

    //read each file in threads
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

    //receive from threads and push to content_string_all
    for text in receiver {
        content_string_all.push_str(&text);
    }

    //Transform String to Vector of each word and trim some chars
    let content_vec: Vec<String> = trim_to_words(content_string_all)?;

    println!("Total number of words read: {:?}", content_vec.len());

    //count and sort words according to frequency
    let word_frequency = count_words(&content_vec)?;
    let words_sorted = sort_map_to_vec(word_frequency)?;

    let words_len = words_sorted.len();

    println!(
        "Counted words in {:?}. Number of unique words: {:?} \n Finding words near:",
        instant.elapsed(),
        words_len
    );

    //count words near each unique word
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

    //format result to print to file.
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

    save_file(to_file, path_dir)?;

    println!(
        "Finished in {:?}! Please see file for results",
        instant.elapsed()
    );

    Ok(())
}
