use std::fs::File;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

// Use the public functions added in src/office.rs
use text_analysis::{extract_text_from_docx, extract_text_from_odt};

fn write_minimal_docx(target: &Path, body: &str) {
    // Minimal DOCX: just a ZIP with "word/document.xml"
    let file = File::create(target).expect("create docx file");
    let mut zip = ZipWriter::new(file);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // It's fine to skip directory entries, but we include "word/" directory for clarity
    zip.add_directory("word", deflated).expect("add word dir");

    let document_xml = format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>{}</w:t></w:r></w:p>
  </w:body>
</w:document>"##,
        body
    );

    zip.start_file("word/document.xml", deflated)
        .expect("start document.xml");
    zip.write_all(document_xml.as_bytes())
        .expect("write document.xml");
    zip.finish().expect("finish docx zip");
}

fn write_minimal_odt(target: &Path, body: &str) {
    // Minimal ODT: a ZIP with "content.xml"
    // Spec requires a "mimetype" first entry stored, but the parser only needs content.xml.
    let file = File::create(target).expect("create odt file");
    let mut zip = ZipWriter::new(file);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let content_xml = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  office:version="1.2">
  <office:body>
    <office:text>
      <text:p>{}</text:p>
    </office:text>
  </office:body>
</office:document-content>"##,
        body
    );

    zip.start_file("content.xml", deflated)
        .expect("start content.xml");
    zip.write_all(content_xml.as_bytes())
        .expect("write content.xml");
    zip.finish().expect("finish odt zip");
}

#[test]
fn docx_roundtrip_minimal_text() {
    let dir = tempdir().expect("create tempdir");
    let path = dir.path().join("sample.docx");
    let body = "Hello DOCX";
    write_minimal_docx(&path, body);

    let extracted = extract_text_from_docx(&path).expect("extract text from docx");
    assert_eq!(extracted, body, "DOCX extraction should match input body");
}

#[test]
fn odt_roundtrip_minimal_text() {
    let dir = tempdir().expect("create tempdir");
    let path = dir.path().join("sample.odt");
    let body = "Hello ODT";
    write_minimal_odt(&path, body);

    let extracted = extract_text_from_odt(&path).expect("extract text from odt");
    assert_eq!(extracted, body, "ODT extraction should match input body");
}

#[test]
fn docx_parsing_handles_line_breaks_and_paragraphs() {
    let dir = tempdir().expect("create tempdir");
    let path = dir.path().join("sample_breaks.docx");

    // One paragraph with an explicit line break, then another paragraph.
    // Parser behavior (from src/office.rs):
    // - <w:br> => '\n'
    // - End of <w:p> => '\n'
    // - normalize_whitespace collapses/normalizes lines and trims.
    let xml = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>Line 1</w:t></w:r>
      <w:r><w:br/></w:r>
      <w:r><w:t>Line 2</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>Para 2</w:t></w:r></w:p>
  </w:body>
</w:document>"##;

    // Build a docx with custom document.xml
    let file = File::create(&path).expect("create docx file");
    let mut zip = ZipWriter::new(file);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.add_directory("word", deflated).expect("add word dir");
    zip.start_file("word/document.xml", deflated)
        .expect("start document.xml");
    zip.write_all(xml.as_bytes()).expect("write document.xml");
    zip.finish().expect("finish docx zip");

    let extracted = extract_text_from_docx(&path).expect("extract text");
    // Expected: "Line 1\nLine 2\nPara 2"
    assert_eq!(extracted, "Line 1\nLine 2\nPara 2");
}

#[test]
fn odt_parsing_handles_paragraphs() {
    let dir = tempdir().expect("create tempdir");
    let path = dir.path().join("sample_breaks.odt");

    // Two paragraphs. Parser behavior (from src/office.rs):
    // - End of <text:p> => '\n'
    // - normalize_whitespace trims trailing newline
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  office:version="1.2">
  <office:body>
    <office:text>
      <text:p>First paragraph</text:p>
      <text:p>Second paragraph</text:p>
    </office:text>
  </office:body>
</office:document-content>"##;

    // Build an odt with custom content.xml
    let file = File::create(&path).expect("create odt file");
    let mut zip = ZipWriter::new(file);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("content.xml", deflated)
        .expect("start content.xml");
    zip.write_all(xml.as_bytes()).expect("write content.xml");
    zip.finish().expect("finish odt zip");

    let extracted = extract_text_from_odt(&path).expect("extract text");
    assert_eq!(extracted, "First paragraph\nSecond paragraph");
}

#[test]
fn docx_missing_file_returns_error() {
    let dir = tempdir().expect("create tempdir");
    let missing = dir.path().join("nope.docx");
    let err = extract_text_from_docx(&missing).unwrap_err();
    assert!(
        err.to_lowercase().contains("open .docx failed")
            || err.to_lowercase().contains("open .docx zip failed"),
        "Unexpected error: {err}"
    );
}

#[test]
fn odt_missing_file_returns_error() {
    let dir = tempdir().expect("create tempdir");
    let missing = dir.path().join("nope.odt");
    let err = extract_text_from_odt(&missing).unwrap_err();
    assert!(
        err.to_lowercase().contains("open .odt failed")
            || err.to_lowercase().contains("open .odt zip failed"),
        "Unexpected error: {err}"
    );
}
