//! Text extraction from the Office formats that are really zipped XML
//! (.docx, .pptx, .xlsx). Enough to let the model read a document the user
//! dropped in; formatting, images and formulas are not preserved.

use std::io::{Cursor, Read};

/// Returns the document's text, or `None` when the extension is not an Office
/// format this module understands.
pub fn extract(ext: &str, bytes: &[u8]) -> Option<String> {
    let parts: fn(&str) -> bool = match ext {
        "docx" => |n| n == "word/document.xml",
        "pptx" => |n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"),
        "xlsx" => |n| n == "xl/sharedStrings.xml",
        _ => return None,
    };
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut names: Vec<String> = (0..zip.len()).filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string())).collect();
    names.sort();
    let mut out = String::new();
    for name in names.iter().filter(|n| parts(n)) {
        let mut xml = String::new();
        if zip.by_name(name).ok()?.take(8 * 1024 * 1024).read_to_string(&mut xml).is_err() {
            continue;
        }
        let text = strip_xml(&xml);
        if !text.trim().is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(text.trim_end());
        }
    }
    Some(out)
}

/// Drops every tag, turning paragraph and cell boundaries into line breaks.
fn strip_xml(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 4);
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else { break };
        let tag = &rest[start + 1..start + end];
        let name = tag.trim_start_matches('/').split([' ', '/']).next().unwrap_or("");
        // Paragraphs, line breaks, tabs, table rows and shared-string items.
        if matches!(name, "w:p" | "a:p" | "w:br" | "w:tr" | "si" | "text:p") {
            out.push('\n');
        } else if matches!(name, "w:tab" | "w:tc") {
            out.push('\t');
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    unescape(&out)
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn docx(document_xml: &str) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            w.start_file("word/document.xml", SimpleFileOptions::default()).unwrap();
            w.write_all(document_xml.as_bytes()).unwrap();
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn reads_paragraphs_from_a_docx() {
        let xml = r#"<w:document><w:body><w:p><w:r><w:t>Quarterly report</w:t></w:r></w:p><w:p><w:r><w:t>Revenue &amp; costs</w:t></w:r></w:p></w:body></w:document>"#;
        let text = extract("docx", &docx(xml)).unwrap();
        assert!(text.contains("Quarterly report"));
        assert!(text.contains("Revenue & costs"));
        assert!(text.lines().count() >= 2);
    }

    #[test]
    fn other_extensions_are_left_alone() {
        assert!(extract("png", b"whatever").is_none());
        assert!(extract("txt", b"whatever").is_none());
    }

    #[test]
    fn a_corrupt_archive_yields_nothing() {
        assert!(extract("docx", b"not a zip").is_none());
    }
}
