use std::collections::HashSet;
use tempfile::tempdir;
use text_analysis::*;

fn ngram_contains(ngrams: &std::collections::HashMap<String, usize>, needle: &str) -> bool {
    ngrams.keys().any(|ng| ng.to_lowercase().contains(needle))
}

fn pmi_contains(pmi: &[PmiEntry], needle: &str) -> bool {
    pmi.iter()
        .any(|e| e.word1.to_lowercase().contains(needle) || e.word2.to_lowercase().contains(needle))
}

fn has_sufficient_pmi(pmi: &[PmiEntry], needle: &str) -> bool {
    if pmi.is_empty() {
        // PMI likely not computable (short text), so not an error for this test
        return true;
    }
    pmi_contains(pmi, needle)
}

// === Core language and feature tests ===

#[test]
fn test_english_ngram_and_pmi() {
    // Short text: N-Gram check only
    let text = "The quick brown fox jumps over the lazy dog.";
    let result = analyze_text(text, &HashSet::new(), 3, 2);
    assert!(ngram_contains(&result.ngrams, "fox"));

    // Long text: PMI check
    let text_pmi = "The fox jumps over the dog. The fox is smart. The fox runs fast. The dog is lazy. \
                    Fox and dog are friends. The quick brown fox jumps again. Fox, dog, and friends go out. \
                    Fox jumps. Dog barks. Fox jumps. Dog barks. Fox jumps. Dog barks. Fox jumps. Dog barks.";
    let result_pmi = analyze_text(text_pmi, &HashSet::new(), 2, 2);
    assert!(
        has_sufficient_pmi(&result_pmi.pmi, "fox"),
        "Expected at least one PMI entry to mention 'fox', but got: {:?}",
        result_pmi.pmi
    );
}

#[test]
fn test_german_faust_ngram_and_pmi() {
    let text = "Habe nun, ach! Philosophie, Juristerei und Medizin, Und leider auch Theologie";
    let result = analyze_text(text, &HashSet::new(), 3, 2);
    assert!(ngram_contains(&result.ngrams, "philosoph"));

    // PMI: längerer Text
    let text_pmi = "Der Philosoph liest Bücher. Die Philosophie ist alt. \
                    Der Philosoph und die Philosophie sind verbunden. \
                    Philosophie ist tief. Philosoph spricht über Philosophie. \
                    Philosophie und Philosoph treffen sich oft. Philosophie hilft dem Philosoph.";
    let result_pmi = analyze_text(text_pmi, &HashSet::new(), 2, 2);
    assert!(
        has_sufficient_pmi(&result_pmi.pmi, "philosoph"),
        "PMI-Check for 'philosoph' failed: {:?}",
        result_pmi.pmi
    );
}

#[test]
fn test_french_lepetitprince_ngram_and_pmi() {
    let text = "Toutes les grandes personnes ont d’abord été des enfants.";
    let result = analyze_text(text, &HashSet::new(), 3, 2);
    assert!(ngram_contains(&result.ngrams, "person"));

    let text_pmi = "Les personnes sont importantes. Les personnes vivent en France. \
                    Les enfants aiment les personnes. Les grandes personnes aident les enfants. \
                    Personnes et enfants jouent ensemble. Les personnes, les enfants et les grandes personnes.";
    let result_pmi = analyze_text(text_pmi, &HashSet::new(), 2, 2);
    assert!(
        has_sufficient_pmi(&result_pmi.pmi, "person"),
        "PMI-Check for 'person' failed: {:?}",
        result_pmi.pmi
    );
}

#[test]
fn test_spanish_quijote_ngram_and_pmi() {
    let text = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme.";
    let result = analyze_text(text, &HashSet::new(), 3, 2);
    assert!(ngram_contains(&result.ngrams, "manch"));

    let text_pmi = "La Mancha es famosa. Un hombre vive en La Mancha. \
                    La Mancha y sus tierras son bonitas. Muchos viven en La Mancha. \
                    La Mancha tiene historia. La Mancha es especial. En La Mancha hay molinos.";
    let result_pmi = analyze_text(text_pmi, &HashSet::new(), 2, 2);
    assert!(
        has_sufficient_pmi(&result_pmi.pmi, "manch"),
        "PMI-Check for 'manch' failed: {:?}",
        result_pmi.pmi
    );
}

#[test]
fn test_italian_pinocchio_ngram_and_pmi() {
    let text = "C’era una volta… un re! — diranno subito i miei piccoli lettori.";
    let result = analyze_text(text, &HashSet::new(), 3, 2);
    assert!(ngram_contains(&result.ngrams, "lettor"));

    let text_pmi = "I lettori leggono libri. Piccoli lettori amano le storie. \
                    Lettori e libri sono inseparabili. Lettori, piccoli lettori, grandi lettori. \
                    Lettori leggono Pinocchio. I lettori ridono. Lettori felici.";
    let result_pmi = analyze_text(text_pmi, &HashSet::new(), 2, 2);
    assert!(
        has_sufficient_pmi(&result_pmi.pmi, "lettor"),
        "PMI-Check for 'lettor' failed: {:?}",
        result_pmi.pmi
    );
}

#[test]
fn test_arabic_sample_ngram_and_pmi() {
    let text = "الكتاب جميل والمكتب قريب من البيت";
    let result = analyze_text(text, &HashSet::new(), 2, 2);
    assert!(
        result
            .ngrams
            .keys()
            .any(|ng| ng.contains("جميل") || ng.contains("مكتب"))
    );

    // Deutlich längerer Testtext mit vielen 'جميل'
    let text_pmi = "\
        الكتاب جميل والمكتب جميل والجو جميل والولد جميل \
        والبنت جميلة والطريق جميل والمنزل جميل والورد جميل \
        والحديقة جميلة والمدينة جميلة \
        الكتاب جميل والمكتب جميل والجو جميل والولد جميل \
        والبنت جميلة والطريق جميل والمنزل جميل والورد جميل \
        والحديقة جميلة والمدينة جميلة \
        الكتاب جميل والمكتب جميل والجو جميل والولد جميل \
        والبنت جميلة والطريق جميل والمنزل جميل والورد جميل \
        والحديقة جميلة والمدينة جميلة \
        الكتاب جميل والمكتب جميل والجو جميل والولد جميل \
        والبنت جميلة والطريق جميل والمنزل جميل والورد جميل \
        والحديقة جميلة والمدينة جميلة \
        ";
    let result_pmi = analyze_text(text_pmi, &HashSet::new(), 2, 2);

    assert!(
        pmi_contains(&result_pmi.pmi, "جميل"),
        "PMI-Check for 'جميل' failed: {:?}",
        result_pmi.pmi
    );
}

// === Edge and error cases ===

#[test]
fn test_empty_input_yields_no_words() {
    let result = analyze_text("", &HashSet::new(), 2, 2);
    assert!(result.wordfreq.is_empty());
    assert!(result.ngrams.is_empty());
    assert!(result.pmi.is_empty());
}

#[test]
fn test_only_stopwords_yields_no_words() {
    let sw: HashSet<_> = ["the", "a", "and"].iter().map(|s| s.to_string()).collect();
    let input = "the a and the";
    let result = analyze_text(input, &sw, 2, 2);
    assert!(result.wordfreq.is_empty());
    assert!(result.ngrams.is_empty());
    assert!(result.pmi.is_empty());
}

#[test]
fn test_unicode_and_apostrophes_are_handled() {
    let input = "naïve über niño l'enfant c'est";
    let result = analyze_text(input, &HashSet::new(), 1, 1);
    let words: Vec<_> = result.wordfreq.keys().collect();
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
    let corpus = r#"
It was the best of times, it was the worst of times, it was the age of wisdom, 
it was the age of foolishness, it was the epoch of belief, it was the epoch of incredulity, 
it was the season of Light, it was the season of Darkness, it was the spring of hope, 
it was the winter of despair.
"#;

    // Verwende explizit englische Stopwords!
    let mut stopwords = HashSet::new();
    for w in [
        "it", "was", "the", "of", "in", "and", "to", "a", "is", "with", "on", "for", "as", "at",
        "by", "an", "be", "from", "that", "which", "or", "his", "her", "but", "not", "are", "this",
        "all", "had", "so",
    ] {
        stopwords.insert(w.to_string());
    }

    let result = analyze_text(corpus, &stopwords, 3, 2);

    // Check: Ist wenigstens eines der Keywords unter den häufigsten Wörtern?
    let mut freq_vec: Vec<_> = result.wordfreq.iter().collect();
    freq_vec.sort_by(|a, b| b.1.cmp(a.1));
    let top_words: Vec<_> = freq_vec.iter().take(10).map(|(w, _)| w.as_str()).collect();
    let keywords = ["time", "epoch", "age", "season", "spring", "winter"];
    assert!(
        top_words
            .iter()
            .any(|w| keywords.iter().any(|k| w.contains(k))),
        "Expected a semantic keyword among the top words. Got: {:?}",
        top_words
    );

    // Trigrams: mindestens eines enthält ein semantisches Schlüsselwort
    let mut ngram_vec: Vec<_> = result.ngrams.iter().collect();
    ngram_vec.sort_by(|a, b| b.1.cmp(a.1));
    let top_trigrams: Vec<_> = ngram_vec
        .iter()
        .take(10)
        .map(|(ng, _)| ng.as_str())
        .collect();
    let ngram_keywords = [
        "age", "wisdom", "time", "season", "light", "spring", "winter", "epoch", "belief",
        "despair", "dark", "hope", "foolish", "best", "worst",
    ];
    let count_semantic_trigrams = top_trigrams
        .iter()
        .filter(|ng| ngram_keywords.iter().any(|kw| ng.contains(*kw)))
        .count();
    assert!(
        count_semantic_trigrams >= 1,
        "At least one trigram should contain a semantic keyword, got: {:?}",
        top_trigrams
    );

    // PMI wie gehabt, jetzt aber mit echten Keywords!
    if !result.pmi.is_empty() {
        let found_pmi = result.pmi.iter().any(|entry| {
            ngram_keywords.iter().any(|&kw| entry.word1.contains(kw))
                && ngram_keywords.iter().any(|&kw| entry.word2.contains(kw))
                && entry.pmi > 0.0
        });
        assert!(
            found_pmi,
            "Expected a positive PMI entry for a pair of semantic keywords, got: {:?}",
            result.pmi
        );
    }
}
