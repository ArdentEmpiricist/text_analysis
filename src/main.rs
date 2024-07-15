//! # Text_Analysis
//! Analyze text stored as *.txt in provided file or directory. Doesn't read files in subdirectories.
//! Counting all words and then searching for every unique word in the vicinity (+-5 words).
//! Stores results in file [date/time]results_word_analysis.txt in given directory.
//! ## Usage: ```text_analysis path/to/directory_or_file```

use std::collections::HashMap;
use std::env::args;
use std::ffi::OsStr;
use std::fs::read_dir;
use std::fs::File;
use std::io::prelude::Read;
use std::panic;
use std::path::PathBuf;
use std::time::Instant;

use text_analysis::{
    count_words, get_index_max, get_index_min, save_file, sort_map_to_vec, trim_to_words,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let instant = Instant::now();

    //get path or filename from args
    let path = PathBuf::from(args().nth(1).expect("no file or directory provided"));

    //print path/file provided to stdout
    println!("path or file: {:?}", path);

    //Vec documents will contain filenames of readable files in directory
    let mut documents = Vec::new();
    //path_dir is the directory to save results file in.
    let mut path_dir: PathBuf = PathBuf::new();
    //Ckeck if argument is a file and push to Vec documents
    if path.is_file() {
        path_dir.push(
            path.parent()
                .expect("error parsing path for provided single file"),
        );
        documents.push(path)
        //Ckeck if argument is a directory
    } else if path.is_dir() {
        path_dir.push(path.clone());
        //walk directory and add .txt to Vec documents - TO DO: Add support for pdf and docx files
        for entry in read_dir(&path).expect("error parsing 'entry in read_dir(&path)'") {
            let entry = entry.expect("error unwrapping entry");
            let path = entry.path();
            if path.is_file()
                && !path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .expect("error transforming filename to str")
                    .contains("results_word_analysis")
                && path.extension().and_then(OsStr::to_str) == Some("txt")
                //|| path.extension().and_then(OsStr::to_str) == Some("pdf") //TO DO: Enable pdf
                //|| path.extension().and_then(OsStr::to_str) == Some("docx") //TO DO: Enable docx
            {
                documents.push(path);
            }
        }
    } else {
        panic!("Provided argument is neither directory nor file. Please check.")
    }
    //prepare Hashmaps to store results
    let mut frequency: HashMap<String, u32> = HashMap::new();

    let mut words_near_vec_map: HashMap<String, Vec<String>> = HashMap::new();

    let mut map_near: HashMap<String, Vec<(String, u32)>> = HashMap::new();

    //read each file and globally update the HashMap "frequency" (frequency of each word) and HashMap "words_near_vec_map" (with Vec of counted words near each word)
    for filename in documents {
        if filename.extension().and_then(OsStr::to_str) == Some("txt") {
            let mut f: File = File::open(filename).expect("error opening txt-file");
            let mut text = String::new();
            f.read_to_string(&mut text).expect("error reading txt-file");
            let content_vec: Vec<String> = trim_to_words(text);
            let mut words_near_vec: Vec<String> = Vec::new();

            for (index, word) in content_vec.clone().into_iter().enumerate() {
                *frequency.entry(word.to_owned()).or_insert(0) += 1;

                let min: usize = get_index_min(&index);
                let max: usize = get_index_max(&index, &content_vec.len());

                (for (number, value) in content_vec.iter().enumerate().take(max).skip(min) {
                    if number == index {
                        continue;
                    } else {
                        //println!("{:?}", content_vec[i]);
                        words_near_vec.push(value.clone()); //pushes -+5 words to vec
                    }
                });

                words_near_vec_map
                    .entry(word.to_owned())
                    .or_default()
                    .append(&mut words_near_vec);
            }
        } else if filename.extension().and_then(OsStr::to_str) == Some("pdf") {
            /* 
            PDF support still shows quite some errors and is prone to panic
            */
            let bytes = std::fs::read(filename).expect("error opening pdf-file");
            let text = pdf_extract::extract_text_from_mem(&bytes).expect("error reading pdf-file");
            let content_vec: Vec<String> = trim_to_words(text);
            let mut words_near_vec: Vec<String> = Vec::new();

            for (index, word) in content_vec.clone().into_iter().enumerate() {
                *frequency.entry(word.to_owned()).or_insert(0) += 1;

                let min: usize = get_index_min(&index);
                let max: usize = get_index_max(&index, &content_vec.len());

                (for (number, value) in content_vec.iter().enumerate().take(max).skip(min) {
                    if number == index {
                        continue;
                    } else {
                        //println!("{:?}", content_vec[i]);
                        words_near_vec.push(value.clone()); //pushes -+5 words to vec
                    }
                });

                words_near_vec_map
                    .entry(word.to_owned())
                    .or_default()
                    .append(&mut words_near_vec);
            }
        } else if filename.extension().and_then(OsStr::to_str) == Some("docx") {
            /* 
            TO DO: Handle *.docx files
            */
            continue;
        } else {
            continue;
        }
    }

    //count Vec with words nears each words
    for (word, words) in words_near_vec_map {
        let counted_near = sort_map_to_vec(count_words(&words));
        map_near.entry(word).or_insert(counted_near);
    }

    //Sort frequency HashMap into Vec
    let counted = sort_map_to_vec(frequency);

    //format output and write to file
    let mut to_file = String::new();
    for (word, frequency) in counted {
        let words_near = &map_near[&word];
        let combined = format!(
            "Word: {:?}, Frequency: {:?},\n Words near: {:?}\n\n",
            word, frequency, words_near
        );
        to_file.push_str(&combined);
    }

    //save results to file in analyzed path, format: ("%Y_%m_%d_%H_%M_%S_results_word_analysis.txt")
    let filename = save_file(to_file, path_dir)?;

    println!(
        "Finished in {:?}! Please see file {:?} for results",
        instant.elapsed(), filename
    );
    Ok(())
}
