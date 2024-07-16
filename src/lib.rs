use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use chrono::prelude::*;

///Splits String into single words as Vector<String>.
///Splits String at whitespaces and removes chars like , or ?. Change the relevant line to remove or add chars from provided String.
/// # Example
/// ```
/// #[test]
/// fn test() {
/// use text_analysiss::trim_to_words;
/// let words = "(_test] {test2!=".to_string();
/// let trimmed = trim_to_words(words).unwrap();
/// let expected = vec!["test".to_string(), "test2".to_string()];
/// assert_eq!(trimmed, expected);
/// }
/// ```
pub fn trim_to_words(content: String) -> std::vec::Vec<std::string::String> {
    let content: Vec<String> = content
        .to_lowercase()
        .replace(&['-'][..], " ")
        //should 's be replaced?
        .replace("'s", "")
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
    content
}
///Takes &Vec<String> and counts the quantity of each word. Returns Hashmap<String,u32>, with String being the word and u32 the quantity
/// # Example
/// ```
/// #[test]
/// fn test_count_words() {
/// use text_analysiss::count_words;
/// let words = vec![
///            "one".to_string(),
///            "two".to_string(),
///            "two".to_string(),
///            "three".to_string(),
///            "three".to_string(),
///            "three".to_string(),
///        ];
///        let counted = count_words(&words);
///        let mut words_map = HashMap::new();
///        words_map.insert("one".to_string(), 1 as u32);
///        words_map.insert("two".to_string(), 2 as u32);
///        words_map.insert("three".to_string(), 3 as u32);
///        assert_eq!(counted, words_map);
/// }
/// ```
pub fn count_words(words: &[String]) -> std::collections::HashMap<std::string::String, u32> {
    let mut frequency: HashMap<String, u32> = HashMap::new();
    for word in words {
        //ignore words constiting of only one char?
        //if word.len() > 1 {
        *frequency.entry(word.to_owned()).or_insert(0) += 1;
        //}
    }
    frequency
}

///Sort words in HashMap<Word, Frequency> according to frequency into Vec<String, u32>.
/// # Example
/// ```
/// use text_analysis::sort_map_to_vec;
/// use std::collections::HashMap;
/// let mut words_map = HashMap::new();
/// words_map.insert("one".to_string(), 1 as u32);
/// words_map.insert("two".to_string(), 2 as u32);
/// words_map.insert("three".to_string(), 3 as u32);
/// let vec_sorted = sort_map_to_vec(words_map);
/// let expected = vec![("three".to_string(), 3 as u32), ("two".to_string(), 2 as u32), ("one".to_string(), 1 as u32)];
/// assert_eq!(vec_sorted, expected);
/// ```
pub fn sort_map_to_vec(
    frequency: HashMap<String, u32>,
) -> std::vec::Vec<(std::string::String, u32)> {
    let mut vec_sorted: Vec<(String, u32)> = frequency.into_iter().collect();
    vec_sorted.sort_by(|a, b| b.1.cmp(&a.1));
    vec_sorted
}

///Get mininum index and guarantee that index is alway >=0
/// # Example
/// ```
///[test];
///fn test() {
///use text_analysis::get_index_min;
///let index1 = 5;
///let min_index1 = get_index_min(&index1);
///assert_eq!(min_index1, 0);
///};
/// ```

pub fn get_index_min(index: &usize) -> usize {
    if *index as isize - 5 < 0 {
        //check if index -5 would result in negative number, return 0 in case
        0
    } else {
        //if index-5 > 0, return index-5
        index - 5
    }
}

///Get maximum index and garantee that index does not exeed total length of Vec
/// # Example
/// ```
/// [test];
/// fn test() {
/// use text_analysis::get_index_max;
/// let index1 = 5;
/// let max_index1 = get_index_max(&index1, &9);
/// assert_eq!(max_index1, 9);
/// };
/// ```
pub fn get_index_max(index: &usize, max_len: &usize) -> usize {
    if index + 5 > *max_len {
        *max_len
    } else {
        index + 5
    }
}

///save file to path. Return result.
pub fn save_file(to_file: String, mut path: PathBuf) -> std::io::Result<PathBuf> {
    let local: DateTime<Local> = Local::now();
    let new_filename: String = local
        .format("%Y_%m_%d_%H_%M_%S_results_word_analysis.txt")
        .to_string();
    path.push(new_filename);

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;

    file.write_all(to_file.as_bytes())?;

    Ok(path)
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
        let counted = count_words(&words);
        let mut words_map = HashMap::new();
        words_map.insert("one".to_string(), 1_u32);
        words_map.insert("two".to_string(), 2_u32);
        words_map.insert("three".to_string(), 3_u32);
        assert_eq!(counted, words_map);
    }

    #[test]
    fn test_max_min_index() {
        let index1 = 5;
        let min_index1 = get_index_min(&index1);
        let max_index1 = get_index_max(&index1, &9);
        assert_eq!(min_index1, 0);
        assert_eq!(max_index1, 9);
        let index2 = 0;
        let min_index2 = get_index_min(&index2);
        let max_index2 = get_index_max(&index2, &5);
        assert_eq!(min_index2, 0);
        assert_eq!(max_index2, 5);
        let index3 = 100;
        let min_index3 = get_index_min(&index3);
        let max_index3 = get_index_max(&index3, &103);
        assert_eq!(min_index3, 95);
        assert_eq!(max_index3, 103);
    }

    #[test]
    fn example_test() {
        use std::time::Instant;
        //start the clock
        let instant = Instant::now();

        let mut frequency: HashMap<String, u32> = HashMap::new();

        let mut words_near_vec_map: HashMap<String, Vec<String>> = HashMap::new();

        let mut map_near: HashMap<String, Vec<(String, u32)>> = HashMap::new();

        let text: String = "An example phrase including two times the word two".to_string();
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
                    words_near_vec.push(value.clone()); //pushes -+5 words to vec
                }
            });

            words_near_vec_map
                .entry(word.to_owned())
                .or_default()
                .append(&mut words_near_vec);
        }

        //count Vec with words nears each words
        for (word, words) in words_near_vec_map {
            let counted_near = sort_map_to_vec(count_words(&words));
            map_near.entry(word).or_insert(counted_near);
        }

        //Sort frequency HashMap into Vec
        let counted = sort_map_to_vec(frequency);

        //format output
        let mut to_file = String::new();
        for (word, frequency) in counted {
            let words_near = &map_near[&word];
            let combined = format!(
                "Word: {:?}, Frequency: {:?},\n Words near: {:?}\n\n",
                word, frequency, words_near
            );
            to_file.push_str(&combined);
        }

        //print time elapsed and output to stdout
        println!(
            "Finished in {:?}! Results:\n {}",
            instant.elapsed(),
            to_file
        );
    }
}
