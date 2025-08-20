use quick_xml::Reader;
use quick_xml::events::Event;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub fn extract_text_from_docx(p: &Path) -> Result<String, String> {
    let file = File::open(p).map_err(|e| format!("Open .docx failed: {e}"))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("Open .docx zip failed: {e}"))?;
    let mut doc = zip
        .by_name("word/document.xml")
        .map_err(|_| "Missing word/document.xml".to_string())?;
    let mut xml = String::new();
    doc.read_to_string(&mut xml)
        .map_err(|e| format!("Read document.xml failed: {e}"))?;
    parse_docx_xml(&xml)
}

pub fn extract_text_from_odt(p: &Path) -> Result<String, String> {
    let file = File::open(p).map_err(|e| format!("Open .odt failed: {e}"))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("Open .odt zip failed: {e}"))?;
    let mut doc = zip
        .by_name("content.xml")
        .map_err(|_| "Missing content.xml".to_string())?;
    let mut xml = String::new();
    doc.read_to_string(&mut xml)
        .map_err(|e| format!("Read content.xml failed: {e}"))?;
    parse_odt_xml(&xml)
}

// ---- Internal helpers ----

fn parse_docx_xml(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut out = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if local_name(e.name().as_ref()) == b"br" {
                    out.push('\n');
                }
            }
            Ok(Event::End(e)) => {
                if local_name(e.name().as_ref()) == b"p" {
                    out.push('\n');
                }
            }
            Ok(Event::Text(t)) => {
                out.push_str(&t.unescape().map_err(|e| e.to_string())?);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Parse .docx XML failed: {e}")),
            _ => {}
        }
        buf.clear();
    }
    Ok(normalize_whitespace(&out))
}

fn parse_odt_xml(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut out = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if matches!(local_name(e.name().as_ref()), b"line-break" | b"br") {
                    out.push('\n');
                }
            }
            Ok(Event::End(e)) => {
                if matches!(local_name(e.name().as_ref()), b"p" | b"h") {
                    out.push('\n');
                }
            }
            Ok(Event::Text(t)) => {
                out.push_str(&t.unescape().map_err(|e| e.to_string())?);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Parse .odt XML failed: {e}")),
            _ => {}
        }
        buf.clear();
    }
    Ok(normalize_whitespace(&out))
}

fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

fn normalize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_blank = false;
    for raw_line in s.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !last_blank {
                out.push('\n');
                last_blank = true;
            }
        } else {
            if !out.is_empty() && !last_blank {
                out.push('\n');
            }
            out.push_str(line);
            out.push('\n');
            last_blank = false;
        }
    }
    out.trim_end().to_string()
}
