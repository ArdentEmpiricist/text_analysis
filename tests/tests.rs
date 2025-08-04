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
