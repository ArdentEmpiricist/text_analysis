//! # Text_Analysis
//! Analyze text stored as *.txt or *pdf in chosen directory. Doesn't read files in subdirectories.
//! Counting all words and then searching for every unique word in the vicinity (+-5 words).
//! Stores results in file [date/time]results_word_analysis.txt
//! ## Usage: ```text_analysis path```
//! # Example
//! ```
//! use text_analysis::{count_words, save_file, sort_map_to_vec, trim_to_words, words_near};
//! use std::collections::HashMap;
//!
//! let content_string: String = "An example phrase including two times the word two".to_string();
//! let content_vec: Vec<String> = trim_to_words(content_string).unwrap();
//!
//! let word_frequency = count_words(&content_vec).unwrap();
//! let words_sorted = sort_map_to_vec(word_frequency).unwrap();
//!
//!
//! let mut index_rang: usize = 0;
//! let mut words_near_map: HashMap<String, HashMap<String, u32>> = HashMap::new();
//! for word in &words_sorted {
//!     words_near_map.extend(words_near(&word, index_rang, &content_vec, &words_sorted).unwrap());
//!     index_rang += 1;
//!     }
//!
//! let mut result_as_string = String::new();
//!
//! for word in words_sorted {
//!     let (word_only, frequency) = &word;
//!     let words_near = &words_near_map[word_only];
//!     let combined = format!(
//!         "Word: {:?}, Frequency: {:?},\nWords near: {:?} \n\n",
//!         word_only,
//!         frequency,
//!         sort_map_to_vec(words_near.to_owned()).unwrap()
//!         );
//!     result_as_string.push_str(&combined);
//! }
//! println!("{:?}", result_as_string);
//!
//! ```

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::path::PathBuf;

use chrono::prelude::*;

use rayon::prelude::*;

///Search for words +-5 around given word. Returns result.
/// # Example
/// ```
/// use std::{collections::HashMap, hash::Hash};
/// use text_analysis::{count_words, sort_map_to_vec, words_near};
/// fn keys_match<T: Eq + Hash, U, V>(map1: &HashMap<T, U>, map2: &HashMap<T, V>) -> bool {
///     map1.len() == map2.len() && map1.keys().all(|k| map2.contains_key(k))
/// }
/// 
/// let words = vec![
///     "one".to_string(),
///     "two".to_string(),
///     "three".to_string(),
///     "four".to_string(),
///     "four".to_string(),
///     "five".to_string(),
/// ];
/// let word = ("two".to_string(), 2 as u32);
/// let words_sorted: Vec<(String, u32)> =
///     sort_map_to_vec(count_words(&words).unwrap()).unwrap();
/// let index_rang: usize = 1;
/// let words_near_map = words_near(&word, index_rang, &words, &words_sorted).unwrap();
/// let mut hashmap_inner = HashMap::new();
/// hashmap_inner.insert("four".to_string(), 2 as u32);
/// hashmap_inner.insert("one".to_string(), 1 as u32);
/// hashmap_inner.insert("three".to_string(), 1 as u32);
/// hashmap_inner.insert("five".to_string(), 1 as u32);
/// let mut expected_map = HashMap::new();
/// expected_map.insert("two".to_string(), hashmap_inner);
/// assert!(keys_match(&words_near_map, &expected_map));
/// ```
pub fn words_near(
    word: &(String, u32),
    index_rang: usize,
    content_vec: &Vec<String>,
    words_sorted: &Vec<(String, u32)>,
) -> std::io::Result<HashMap<String, HashMap<String, u32>>> {
    let index: Vec<usize> = positions(&content_vec, &words_sorted[index_rang].0);
    let mut vec_word = Vec::new();
    let max_len = content_vec.len();
    for index_single in index {
        for i in 0..max_len {
            let min: usize = get_index_min(&index_single)?;
            let max: usize = get_index_max(&index_single, &max_len)?;
            if i >= min && i <= max && i != index_single {
                vec_word.push(content_vec[i].clone());
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

///Search for all position (usize) of word in given Vector<String>
/// # Example
/// ```
/// #[test]
/// fn test() {
/// use std::{collections::HashMap, hash::Hash};
/// use text_analysis::{positions};
/// let words = vec![
///     "one".to_string(),
///     "two".to_string(),
///     "three".to_string(),
///     "four".to_string(),
///     "four".to_string(),
///     "five".to_string(),
/// ];
/// let word = "four".to_string();
/// let position = positions(&words, &word);
/// let expected = vec![3,4];
/// assert_eq!(position, expected);
/// }
/// ```
fn positions(vector: &Vec<String>, target: &String) -> Vec<usize> {
    let mut res = Vec::new();
    for (index, c) in vector.into_iter().enumerate() {
        if &c == &target {
            res.push(index)
        }
    }
    res
}

///Count words included in given &Vec<String>. Returns result as HashMap with <Word as String, Count as u32>. Returns result.
/// # Example
/// ```
/// use text_analysis::count_words;
/// use std::collections::HashMap;
/// let words = vec!["one".to_string(),"two".to_string(),"two".to_string(),"three".to_string(),"three".to_string(),"three".to_string(),];
/// let counted = count_words(&words).unwrap();
/// let mut words_map = HashMap::new();
/// words_map.insert("one".to_string(), 1 as u32);
/// words_map.insert("two".to_string(), 2 as u32);
/// words_map.insert("three".to_string(), 3 as u32);
/// assert_eq!(counted, words_map);
/// ```
pub fn count_words(words: &Vec<String>) -> std::io::Result<HashMap<String, u32>> {
    let mut frequency: HashMap<String, u32> = HashMap::new();
    for word in words {
        //ignore words constiting of only one char
        if word.len() > 1 {
            *frequency.entry(word.to_owned()).or_insert(0) += 1;
        }
    }
    Ok(frequency)
}

///Sort words in HashMap<Word, Frequency> according to frequency  into Vector. Returns result.
/// # Example
/// ```
/// use text_analysis::sort_map_to_vec;
/// use std::collections::HashMap;
/// let mut words_map = HashMap::new();
/// words_map.insert("one".to_string(), 1 as u32);
/// words_map.insert("two".to_string(), 2 as u32);
/// words_map.insert("three".to_string(), 3 as u32);
/// let vec_sorted = sort_map_to_vec(words_map).unwrap();
/// let expected = vec![("three".to_string(), 3 as u32), ("two".to_string(), 2 as u32), ("one".to_string(), 1 as u32)];
/// assert_eq!(vec_sorted, expected);
/// ```
pub fn sort_map_to_vec(frequency: HashMap<String, u32>) -> std::io::Result<Vec<(String, u32)>> {
    let mut vec_sorted: Vec<(String, u32)> = frequency.into_par_iter().collect();
    vec_sorted.par_sort_by(|a, b| b.1.cmp(&a.1));
    Ok(vec_sorted)
}

///Splits content of file into singe words as Vector<String>. Returns result.
/// # Example
/// ```
/// #[test]
/// fn test() {
/// use text_analysis::trim_to_words;
/// let words = "(_test] {test2!=".to_string();
/// let trimmed = trim_to_words(words).unwrap();
/// let expected = vec!["test".to_string(), "test2".to_string()];
/// assert_eq!(trimmed, expected);
/// }
/// ```
pub fn trim_to_words(content: String) -> std::io::Result<Vec<String>> {
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

///Get mininum index.
/// # Example
/// ```
///#[test]
///fn test() {
///use text_analysis::get_index_min;
///let index1 = 5;
///let min_index1 = get_index_min(&index1).unwrap();
///assert_eq!(min_index1, 0);
///}
/// ```
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

///Get maximum index.
/// # Example
/// ```
/// #[test]
/// fn test() {
/// use text_analysis::get_index_max;
/// let index1 = 5;
/// let max_index1 = get_index_max(&index1, &9).unwrap();
/// assert_eq!(max_index1, 9);
/// }
/// ```
fn get_index_max(index: &usize, max_len: &usize) -> std::io::Result<usize> {
    let max = if index + 5 > *max_len {
        *max_len as usize
    } else {
        index + 5
    };
    Ok(max)
}

///save file to path. Return result.
pub fn save_file(to_file: String, mut path: PathBuf) -> std::io::Result<()> {
    let local: DateTime<Local> = Local::now();
    let new_filename: String = local
        .format("%Y_%m_%d_%H_%M_%S_results_word_analysis.txt")
        .to_string();
    path.push(new_filename);

    let mut file = OpenOptions::new().write(true).create(true).open(path)?;

    file.write_all(to_file.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_count() {
        let words = vec![
            "one".to_string(),
            "two".to_string(),
            "two".to_string(),
            "three".to_string(),
            "three".to_string(),
            "three".to_string(),
        ];
        let counted = count_words(&words).unwrap();
        let mut words_map = HashMap::new();
        words_map.insert("one".to_string(), 1 as u32);
        words_map.insert("two".to_string(), 2 as u32);
        words_map.insert("three".to_string(), 3 as u32);
        assert_eq!(counted, words_map);
    }

    #[test]
    fn test_max_min_index() {
        let index1 = 5;
        let min_index1 = get_index_min(&index1).unwrap();
        let max_index1 = get_index_max(&index1, &9).unwrap();
        assert_eq!(min_index1, 0);
        assert_eq!(max_index1, 9);
        let index2 = 0;
        let min_index2 = get_index_min(&index2).unwrap();
        let max_index2 = get_index_max(&index2, &5).unwrap();
        assert_eq!(min_index2, 0);
        assert_eq!(max_index2, 5);
        let index3 = 100;
        let min_index3 = get_index_min(&index3).unwrap();
        let max_index3 = get_index_max(&index3, &103).unwrap();
        assert_eq!(min_index3, 95);
        assert_eq!(max_index3, 103);
    }

    #[test]
    fn test_words_near() {
        use std::{collections::HashMap, hash::Hash};
        fn keys_match<T: Eq + Hash, U, V>(map1: &HashMap<T, U>, map2: &HashMap<T, V>) -> bool {
            map1.len() == map2.len() && map1.keys().all(|k| map2.contains_key(k))
        }

        let words = vec![
            "one".to_string(),
            "two".to_string(),
            "three".to_string(),
            "four".to_string(),
            "four".to_string(),
            "five".to_string(),
        ];
        let word = ("two".to_string(), 2 as u32);
        let words_sorted: Vec<(String, u32)> =
            sort_map_to_vec(count_words(&words).unwrap()).unwrap();
        let index_rang: usize = 1;
        let words_near_map = words_near(&word, index_rang, &words, &words_sorted).unwrap();
        let mut hashmap_inner = HashMap::new();
        hashmap_inner.insert("four".to_string(), 2 as u32);
        hashmap_inner.insert("one".to_string(), 1 as u32);
        hashmap_inner.insert("three".to_string(), 1 as u32);
        hashmap_inner.insert("five".to_string(), 1 as u32);
        let mut expected_map = HashMap::new();
        expected_map.insert("two".to_string(), hashmap_inner);
        assert!(keys_match(&words_near_map, &expected_map));
    }

    #[test]
    fn example_test() {
        let content_string: String =
            "An example phrase including two times the word two".to_string();
        let content_vec: Vec<String> = trim_to_words(content_string).unwrap();

        let word_frequency = count_words(&content_vec).unwrap();
        let words_sorted = sort_map_to_vec(word_frequency).unwrap();

        let mut index_rang: usize = 0;
        let mut words_near_map: HashMap<String, HashMap<String, u32>> = HashMap::new();
        for word in &words_sorted {
            words_near_map
                .extend(words_near(&word, index_rang, &content_vec, &words_sorted).unwrap());

            index_rang += 1;
        }

        let mut result_as_string = String::new();

        for word in words_sorted {
            let (word_only, frequency) = &word;
            let words_near = &words_near_map[word_only];
            let combined = format!(
                "Word: {:?}, Frequency: {:?},\nWords near: {:?} \n\n",
                word_only,
                frequency,
                sort_map_to_vec(words_near.to_owned()).unwrap()
            );
            result_as_string.push_str(&combined);
        }
    }
}
