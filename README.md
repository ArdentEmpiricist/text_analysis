# Text_Analysis

[![Rust](https://github.com/LazyEmpiricist/text_analysis/workflows/Rust/badge.svg?branch=main)](https://github.com/LazyEmpiricist/text_analysis)
[![Crates.io](https://img.shields.io/crates/v/text_analysis)](https://crates.io/crates/text_analysis)
[![Documentation](https://docs.rs/text_analysis/badge.svg)](https://docs.rs/text_analysis/)
[![Crates.io](https://img.shields.io/crates/l/text_analysis)](https://github.com/LazyEmpiricist/text_analysis/blob/main/LICENSE)


Analyze text stored as *.pdf and *.txt in chosen file or directory. Doesn't read files in subdirectories.
Counting all words and then searching for every unique word in the vicinity (+-5 words).
Stores results in file [date/time]results_word_analysis.txt in given directory.

Uses chrono (https://crates.io/crates/chrono) to track time.

**Warning:** Doesn't ouput error if files could not be read and errors are subsequently ignored. May panic at (oddly formated) PDF-files.

**To install:** clone the repository and build from source or use ```cargo install text_analysis```.

**Usage:**
```
text_analysis path/to/directory_or_file
```
**Breaking Change in 0.2:** No longer reads pdfs. Any help to parse *.pdf and *.docx more than welcome.

**Breaking Change in 0.3:** PDF support is back using the crate "pdf-extract", although reading PDFs is still prone to error (and panics). Any help to improve PDF-support and how to parse *.docx is more than welcome.

## Example 

```rust
use text_analysis::{count_words, sort_map_to_vec, trim_to_words, words_near};
use std::collections::HashMap;

use std::time::Instant;
//start the clock
let instant = Instant::now();

//create HashMaps to store results
let mut frequency: HashMap<String, u32> = HashMap::new();
let mut words_near_vec_map: HashMap<String, Vec<String>> = HashMap::new();
let mut map_near: HashMap<String, Vec<(String, u32)>> = HashMap::new();

//create example string to parse
let text: String = "An example phrase including two times the word two".to_string();
//create Vec with parsed words
let content_vec: Vec<String> = trim_to_words(text);
//prepare Vec to store words near each word
let mut words_near_vec: Vec<String> = Vec::new();

//push words to HashMaps
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
        .or_insert_with(Vec::new)
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

```

***Output for "gender" in Butler, Judith. “Performative Acts and Gender Constitution: An Essay in Phenomenology and Feminist Theory.” Theatre Journal, vol. 40, no. 4, 1988, pp. 519–531:***
```
Word: "gender", Frequency: 123,

Words near: [("of", 71), ("the", 68), ("is", 57), ("and", 46), ("that", 31), ("as", 23), ("which", 23), ("acts", 19), ("an", 18), ("to", 17), ("in", 16), ("gender", 14), ("performance", 13), ("sex", 12), ("identity", 12), ("or", 11), ("on", 10), ("be", 10), ("through", 9), ("constitution", 9), ("not", 8), ("for", 8), ("performative", 8), ("with", 7), ("reality", 7), ("if", 7), ("there", 7), ("act", 6), ("it", 6), ("what", 6), ("at", 6), ("this", 6), ("theory", 6), ("by", 5), ("expressive", 5), ("from", 5), ("feminist", 5), ("constituted", 5), ("are", 5), ("social", 5), ("discrete", 5), ("about", 5), ("ones", 4), ("ostensibly", 4), ("identities", 4), ("has", 4), ("various", 4), ("express", 4), ("indeed", 4), ("existing", 4), ("because", 4), ("all", 4), ("understood", 4), ("view", 4), ("cultural", 4), ("itself", 3), ("but", 3), ("sense", 3), ("performances", 3), ("situation", 3), ("description", 3), ("radical", 3), ("binary", 3), ("suggests", 3), ("one", 3), ("reproduction", 3), ("corporeal", 3), ("no", 3), ("political", 3), ("consequence", 3), ("does", 3), ("my", 3), ("sexuality", 3), ("without", 3), ("attributes", 3), ("norms", 2), ("performing", 2), ("kinship", 2), ("might", 2), ("cannot", 2), ("aspires", 2), ("historical", 2), ("ground", 2), ("further", 2), ("any", 2), ("creates", 2), ("possibilities", 2), ("will", 2), ("implicitly", 2), ("doing", 2), ("nothing", 2), ("accomplishment", 2), ("both", 2), ("phenomenological", 2), ("concealed", 2), ("least", 2), ("repetition", 2), ("butler", 2), ("redescribes", 2), ("body", 2), ("then", 2), ("its", 2), ("transformation", 2), ("regulation", 2), ("natural", 2), ("variations", 2), ("way", 2), ("popular", 2), ("instituted", 2), ("our", 2), ("although", 2), ("conception", 2), ("their", 2), ("appears", 2), ("postulation", 2), ("thus", 2), ("category", 2), ("senses", 2), ("do", 2), ("clear", 2), ("control", 2), ("own", 2), ("unrealized", 2), ("idea", 2), ("categories", 2), ("ways", 2), ("within", 2), ("ontological", 2), ("public", 2), ("essentialism", 2), ("biological", 2), ("invariably", 2), ("model", 2), ("stylized", 2), ("constitute", 2), ("real", 2), ("would", 2), ("account", 2), ("fact", 2), ("residual", 1), ("wittig", 1), ("point", 1), ("become", 1), ("521", 1), ("post", 1), ("crucial", 1), ("punished", 1), ("used", 1), ("should", 1), ("level", 1), ("articles", 1), ("conform", 1), ("interesting", 1), ("off", 1), ("yale", 1), ("significance", 1), ("challenges", 1), ("intractable", 1), ("fail", 1), ("manner", 1), ("found", 1), ("falsity", 1), ("victor", 1), ("only", 1), ("ritualized", 1), ("bear", 1), ("naturalized", 1), ("judith", 1), ("approach", 1), ("policy", 1), ("expectation", 1), ("seems", 1), ("can", 1), ("made", 1), ("history", 1), ("politics", 1), ("comply", 1), ("construction", 1), ("affair", 1), ("improvisations", 1), ("produces", 1), ("phenomenology", 1), ("ideal", 1), ("merely", 1), ("usually", 1), ("heterosexual", 1), ("chicago", 1), ("substantial", 1), ("put", 1), ("we", 1), ("always", 1), ("nor", 1), ("life", 1), ("censorship", 1), ("envision", 1), ("choice", 1), ("529", 1), ("1978", 1), ("strategy", 1), ("physical", 1), ("products", 1), ("acting", 1), ("duress", 1), ("assimilated", 1), ("philosophy", 1), ("know", 1), ("created", 1), ("moves", 1), ("polarized", 1), ("527", 1), ("theatrical", 1), ("either", 1), ("too", 1), ("wrong", 1), ("frame", 1), ("similarities", 1), ("242", 1), ("display", 1), ("culturally", 1), ("interpreted", 1), ("truth", 1), ("pose", 1), ("however", 1), ("show", 1), ("separate", 1), ("178", 1), ("objective", 1), ("production", 1), ("expectations", 1), ("complies", 1), ("pre", 1), ("constructed", 1), ("illusion", 1), ("sedimentation", 1), ("explicitly", 1), ("rather", 1), ("contemporary", 1), ("vocabulary", 1), ("stabilized", 1), ("phenomena", 1), ("instance", 1), ("sedimented", 1), ("create", 1), ("something", 1), ("iii", 1), ("effect", 1), ("hence", 1), ("fiction", 1), ("essence", 1), ("university", 1), ("interested", 1), ("many", 1), ("spivak", 1), ("scene", 1), ("under", 1), ("system", 1), ("relations", 1), ("embodied", 1), ("rely", 1), ("episteme", 1), ("prescriptive", 1), ("disguises", 1), ("turn", 1), ("regularly", 1), ("ponty", 1), ("dramatized", 1), ("character", 1), ("large", 1), ("aspect", 1), ("525", 1), ("number", 1), ("conventions", 1), ("prior", 1), ("broadly", 1), ("women", 1), ("set", 1), ("causal", 1), ("books", 1), ("consider", 1), ("quite", 1), ("tacitly", 1), ("examine", 1), ("individual", 1), ("requires", 1), ("conditions", 1), ("maintaining", 1), ("punitive", 1), ("initiates", 1), ("distinguishing", 1), ("1983", 1), ("variously", 1), ("play", 1), ("words", 1), ("arrived", 1), ("am", 1), ("known", 1), ("preserves", 1), ("originate", 1), ("right", 1), ("second", 1), ("restriction", 1), ("expected", 1), ("surface", 1), ("certain", 1), ("thinking", 1), ("transvestites", 1), ("specific", 1), ("existentialist", 1), ("523", 1), ("above", 1), ("survival", 1), ("accord", 1), ("reified", 1), ("scripted", 1), ("signifiers", 1), ("distinction", 1), ("externalizes", 1), ("associated", 1), ("series", 1), ("stylization", 1), ("foucault", 1), ("me", 1), ("framework", 1), ("expresses", 1), ("sign", 1), ("beauvoir", 1), ("belief", 1), ("primarily", 1), ("new", 1), ("occurs", 1), ("531", 1), ("presuppositions", 1), ("familiar", 1), ("gestures", 1), ("construed", 1), ("enacted", 1), ("ought", 1), ("style", 1), ("temporality", 1), ("criticism", 1), ("85", 1), ("live", 1), ("disputed", 1), ("called", 1), ("imagination", 1), ("she", 1), ("rendered", 1), ("strategic", 1), ("facticity", 1), ("patriarchy", 1), ("embodiment", 1), ("theorists", 1), ("core", 1), ("heterosexuality", 1), ("imagine", 1), ("sustain", 1), ("acknowledge", 1), ("becomes", 1), ("between", 1), ("once", 1), ("contradicts", 1), ("revealed", 1), ("correlate", 1), ("contest", 1), ("genealogy", 1), ("drag", 1), ("mckenna", 1), ("significantly", 1), ("authors", 1), ("anthropologist", 1), ("conceptions", 1), ("conceal", 1), ("critique", 1), ("1974", 1), ("merleau", 1), ("given", 1), ("argue", 1), ("regulatory", 1), ("ethnomethodological", 1), ("kind", 1), ("experiences", 1), ("sex12", 1), ("passively", 1), ("perform", 1), ("wendy", 1), ("means", 1), ("essential", 1), ("action", 1), ("serves", 1), ("deal", 1), ("rubin", 1), ("526", 1), ("arrangements", 1), ("non", 1), ("have", 1), ("sustained", 1), ("compelled", 1), ("insist", 1), ("physiology", 1), ("modality", 1), ("tradition1", 1), ("along", 1), ("critical", 1), ("such", 1), ("essay", 1), ("structuralist", 1), ("unwarranted", 1), ("those", 1), ("conceived", 1), ("comprehensive", 1), ("being", 1), ("basically", 1), ("links", 1), ("woman", 1), ("distorted", 1), ("enough", 1), ("fails", 1), ("discussion", 1), ("project", 1), ("perceived", 1), ("after", 1), ("overwhelming", 1), ("beauvoirs", 1), ("readily", 1), ("serve", 1), ("neither", 1), ("reproduce", 1), ("structures", 1), ("aim", 1), ("sketched", 1), ("applied", 1), ("entranced", 1), ("who", 1), ("38", 1), ("cores", 1), ("interpretation", 1), ("fully", 1), ("socially", 1), ("gayatri", 1), ("univocal", 1), ("offers", 1), ("unexamined", 1), ("innovative", 1), ("beyond", 1), ("limits", 1), ("marks", 1), ("assumptions", 1), ("clearly", 1), ("contention", 1), ("regulate", 1), ("world", 1), ("must", 1), ("field", 1), ("so", 1), ("contexts", 1), ("extension", 1), ("true", 1), ("peculiar", 1), ("complexity", 1), ("distinct", 1), ("scathing", 1)] 
```


## To do:
- [x] Read *.txt
- [x] Scan given directory
- [x] Add more comments
- [x] Write tests
- [x] Enable single file as argument
- [x] Read *pdf
- [ ] Show list of read-errors / files couldn't be read
- [ ] Read *.odt, *.doc and *.docx
- [ ] Scan subdirectories

**Help needed to implement and to improve parsing of .pdf and .docx files.**

**Issues and feedback are highly appreciated.** 
