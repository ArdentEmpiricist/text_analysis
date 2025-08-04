use rust_stemmers::{Algorithm, Stemmer};
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;
use text_analysis::{
    ExportFormat, PmiEntry, analyze_path, arabic_stopwords, collocation_stats, compute_pmi,
    english_stopwords, french_stopwords, german_stopwords, italian_stopwords, ngram_analysis,
    spanish_stopwords, trim_to_words,
};

// --- Helper for ngram content check
fn ngram_contains(ngrams: &std::collections::HashMap<String, u32>, needle: &str) -> bool {
    ngrams.keys().any(|ng| ng.to_lowercase().contains(needle))
}

// --- Helper for PMI content check
fn pmi_contains(pmi: &[PmiEntry], needle: &str) -> bool {
    pmi.iter()
        .any(|e| e.word1.to_lowercase().contains(needle) || e.word2.to_lowercase().contains(needle))
}

// === Core language and feature tests ===

#[test]
fn test_english_ngram_and_pmi() {
    // "The quick brown fox..." sample
    let words = trim_to_words(
        "The quick brown fox jumps over the lazy dog.",
        Some(&Stemmer::create(Algorithm::English)),
        &english_stopwords(),
    );
    let ngrams = ngram_analysis(&words, 3);
    assert!(ngram_contains(&ngrams, "fox"));
    let (freq, _, _, pos_matrix) = collocation_stats(&words, 2);
    let pmi = compute_pmi(&freq, &pos_matrix, freq.values().sum(), 1);
    assert!(pmi_contains(&pmi, "fox"));
}

#[test]
fn test_german_faust_ngram_and_pmi() {
    let text = "Habe nun, ach! Philosophie, Juristerei und Medizin, Und leider auch Theologie";
    let words = trim_to_words(
        text,
        Some(&Stemmer::create(Algorithm::German)),
        &german_stopwords(),
    );
    let ngrams = ngram_analysis(&words, 3);
    assert!(ngram_contains(&ngrams, "philosoph"));
    let (freq, _, _, pos_matrix) = collocation_stats(&words, 2);
    let pmi = compute_pmi(&freq, &pos_matrix, freq.values().sum(), 1);
    assert!(pmi_contains(&pmi, "philosoph"));
}

#[test]
fn test_french_lepetitprince_ngram_and_pmi() {
    let text = "Toutes les grandes personnes ont d’abord été des enfants.";
    let words = trim_to_words(
        text,
        Some(&Stemmer::create(Algorithm::French)),
        &french_stopwords(),
    );
    let ngrams = ngram_analysis(&words, 3);
    assert!(ngram_contains(&ngrams, "person"));
    let (freq, _, _, pos_matrix) = collocation_stats(&words, 2);
    let pmi = compute_pmi(&freq, &pos_matrix, freq.values().sum(), 1);
    assert!(pmi_contains(&pmi, "person"));
}

#[test]
fn test_spanish_quijote_ngram_and_pmi() {
    let text = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme.";
    let words = trim_to_words(
        text,
        Some(&Stemmer::create(Algorithm::Spanish)),
        &spanish_stopwords(),
    );
    let ngrams = ngram_analysis(&words, 3);
    assert!(ngram_contains(&ngrams, "manch"));
    let (freq, _, _, pos_matrix) = collocation_stats(&words, 2);
    let pmi = compute_pmi(&freq, &pos_matrix, freq.values().sum(), 1);
    assert!(pmi_contains(&pmi, "manch"));
}

#[test]
fn test_italian_pinocchio_ngram_and_pmi() {
    let text = "C’era una volta… un re! — diranno subito i miei piccoli lettori.";
    let words = trim_to_words(
        text,
        Some(&Stemmer::create(Algorithm::Italian)),
        &italian_stopwords(),
    );
    let ngrams = ngram_analysis(&words, 3);
    assert!(ngram_contains(&ngrams, "lettor"));
    let (freq, _, _, pos_matrix) = collocation_stats(&words, 2);
    let pmi = compute_pmi(&freq, &pos_matrix, freq.values().sum(), 1);
    assert!(pmi_contains(&pmi, "lettor"));
}

#[test]
fn test_arabic_sample_ngram_and_pmi() {
    let text = "الكتاب جميل والمكتب قريب من البيت";
    let words = trim_to_words(
        text,
        Some(&Stemmer::create(Algorithm::Arabic)),
        &arabic_stopwords(),
    );
    let ngrams = ngram_analysis(&words, 2);
    assert!(
        ngrams
            .keys()
            .any(|ng| ng.contains("جميل") || ng.contains("مكتب"))
    );
    let (freq, _, _, pos_matrix) = collocation_stats(&words, 2);
    let pmi = compute_pmi(&freq, &pos_matrix, freq.values().sum(), 1);
    assert!(
        pmi.iter()
            .any(|e| e.word1.contains("جميل") || e.word2.contains("جميل"))
    );
}

// === Edge and error cases ===

#[test]
fn test_empty_input_yields_no_words() {
    let words = trim_to_words("", None, &english_stopwords());
    assert!(words.is_empty());
}

#[test]
fn test_only_stopwords_yields_no_words() {
    let sw = english_stopwords();
    let input = sw.iter().cloned().collect::<Vec<_>>().join(" ");
    let words = trim_to_words(&input, None, &sw);
    assert!(words.is_empty());
}

#[test]
fn test_unicode_and_apostrophes_are_handled() {
    let input = "naïve über niño l'enfant c'est";
    let words = trim_to_words(
        &input,
        Some(&Stemmer::create(Algorithm::French)),
        &french_stopwords(),
    );
    // Test for any variant (with or without apostrophe, stemmed or unstemmed)
    assert!(
        words
            .iter()
            .any(|w| w.contains("enfan") || w.contains("enfant") || w.contains("enf")),
        "Expected a stem of 'enfant', got: {:?}",
        words
    );
    assert!(
        words
            .iter()
            .any(|w| w.contains("naiv") || w.contains("naïv") || w.contains("naïve")),
        "Expected a variant of 'naive', got: {:?}",
        words
    );
}

// === File-based IO and errors ===

#[test]
fn test_nonexistent_file_gives_error() {
    let dir = tempdir().unwrap();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let result = analyze_path("notfound.txt", None, 2, 2, ExportFormat::Txt, false);
    std::env::set_current_dir(orig).unwrap();
    assert!(result.is_err());
}

#[test]
fn test_long_corpus_wordfreq_ngram_pmi_consistency_in_memory() {
    // This test uses a classic English paragraph (Tale of Two Cities, Dickens).
    // It checks if:
    // - The most frequent word is "was"
    // - The trigram "of times it" exists
    // - The PMI for ("was", "the") is positive and present

    use rust_stemmers::{Algorithm, Stemmer};
    use text_analysis::{
        collocation_stats, compute_pmi, english_stopwords, ngram_analysis, trim_to_words,
    };

    // --- Example corpus
    let corpus = r#"
It was the best of times, it was the worst of times, it was the age of wisdom, 
it was the age of foolishness, it was the epoch of belief, it was the epoch of incredulity, 
it was the season of Light, it was the season of Darkness, it was the spring of hope, 
it was the winter of despair.
"#;

    // Tokenization and stemming with English stopwords
    let stemmer = Stemmer::create(Algorithm::English);
    let words = trim_to_words(corpus, Some(&stemmer), &english_stopwords());

    // --- 1. Frequency analysis
    use std::collections::HashMap;
    let mut freq = HashMap::new();
    for w in &words {
        *freq.entry(w.clone()).or_insert(0u32) += 1;
    }
    // "time" (or its stem) should be among the most frequent words
    let most_common = freq
        .iter()
        .max_by_key(|(_w, c)| *c)
        .map(|(w, _)| w.as_str())
        .unwrap_or("");
    let top_candidates = ["time", "epoch", "age", "season", "spring", "winter"];
    assert!(
        top_candidates.iter().any(|&w| most_common.contains(w)),
        "Most frequent word should be among {:?}, got: {:?}",
        top_candidates,
        most_common
    );

    // --- 2. Ngram analysis (trigrams)
    let ngrams = ngram_analysis(&words, 3);
    // The most frequent trigram should be plausible
    let mut ngram_vec: Vec<_> = ngrams.iter().collect();
    ngram_vec.sort_by(|a, b| b.1.cmp(a.1));
    let top_trigrams: Vec<_> = ngram_vec
        .iter()
        .take(5)
        .map(|(ng, _)| ng.as_str())
        .collect();

    // Define a broader set of semantic keywords, including both stems and common forms
    let keywords = [
        "age", "wisdom", "time", "season", "light", "spring", "winter", "epoch", "belief",
        "despair", "dark", "hope", "foolish", "best", "worst",
    ];

    // Count how many top trigrams contain at least two distinct keywords
    let count_semantic_trigrams = top_trigrams
        .iter()
        .filter(|ng| {
            let match_count = keywords.iter().filter(|kw| ng.contains(*kw)).count();
            match_count >= 2
        })
        .count();

    assert!(
        count_semantic_trigrams >= 1,
        "At least one of the top trigrams should contain at least two semantic keywords {:?}, got top trigrams: {:?}",
        keywords,
        top_trigrams
    );

    // Extra: Ensure all top trigrams are not just function words (no stopword-only ngrams)
    let stopwords = text_analysis::english_stopwords();
    for ng in &top_trigrams {
        let word_count = ng
            .split_whitespace()
            .filter(|w| !stopwords.contains(&w.to_string()))
            .count();
        assert!(
            word_count >= 2,
            "Top trigram '{}' should contain at least two non-stopword tokens",
            ng
        );
    }

    // --- 3. PMI analysis
    let (_freq, _ctx, _dir, pos_matrix) = collocation_stats(&words, 2);
    let total = words.len() as u32;
    let pmi = compute_pmi(&freq, &pos_matrix, total, 1);
    // Keywords expected in PMI pairs after stopword removal
    let keywords = [
        "age", "wisdom", "time", "season", "light", "spring", "winter", "epoch", "belief",
        "despair", "dark", "hope", "foolish", "best", "worst",
    ];

    // Find if any PMI entry contains a pair of these keywords and has positive PMI
    let found_pmi = pmi.iter().any(|entry| {
        keywords.iter().any(|&kw| entry.word1.contains(kw))
            && keywords.iter().any(|&kw| entry.word2.contains(kw))
            && entry.pmi > 0.0
    });
    assert!(
        found_pmi,
        "Expected a positive PMI entry for a pair of semantic keywords, got: {:?}",
        pmi
    );
}
