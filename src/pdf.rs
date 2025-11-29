#[cfg(feature = "pdf")]
use lopdf::Document;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// PDF document information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PdfInfo {
    /// Extracted text content
    pub text: String,

    /// Number of pages
    pub num_pages: usize,

    /// PDF metadata
    pub metadata: PdfMetadata,
}

/// PDF metadata
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PdfMetadata {
    /// Document title
    pub title: Option<String>,

    /// Document author
    pub author: Option<String>,

    /// Document subject
    pub subject: Option<String>,

    /// Creator application
    pub creator: Option<String>,

    /// Producer (PDF generator)
    pub producer: Option<String>,

    /// Creation date
    pub creation_date: Option<String>,

    /// Modification date
    pub modification_date: Option<String>,
}

/// Extract text from PDF bytes
#[cfg(feature = "pdf")]
pub fn extract_pdf_text(pdf_bytes: &[u8], max_pages: Option<usize>) -> Result<PdfInfo> {
    let doc = Document::load_mem(pdf_bytes).context("Failed to load PDF document")?;

    let num_pages = doc.get_pages().len();
    let pages_to_extract = max_pages.unwrap_or(num_pages).min(num_pages);

    // Extract text from specified number of pages
    let mut all_text = String::new();

    for page_num in 1..=pages_to_extract {
        match extract_page_text(&doc, page_num as u32) {
            Ok(text) => {
                all_text.push_str(&text);
                all_text.push_str("\n\n");
            }
            Err(e) => {
                tracing::warn!("Failed to extract text from page {}: {}", page_num, e);
            }
        }
    }

    // Extract metadata
    let metadata = extract_pdf_metadata(&doc);

    Ok(PdfInfo {
        text: all_text.trim().to_string(),
        num_pages,
        metadata,
    })
}

/// Extract text from a specific page
#[cfg(feature = "pdf")]
fn extract_page_text(doc: &Document, page_num: u32) -> Result<String> {
    let pages = doc.get_pages();
    let page_id = pages
        .get(&page_num)
        .context(format!("Page {} not found", page_num))?;

    let contents = doc
        .get_page_content(*page_id)
        .context("Failed to get page content")?;

    let mut text = String::new();

    // Simple text extraction from content stream
    // This is a basic implementation - for production, consider using pdf-extract crate
    let content_str = String::from_utf8_lossy(&contents);

    // Look for text between BT (Begin Text) and ET (End Text) operators
    for line in content_str.lines() {
        if line.contains("Tj") || line.contains("TJ") {
            // Extract text from text showing operators
            if let Some(text_content) = extract_text_from_operator(line) {
                text.push_str(&text_content);
                text.push(' ');
            }
        }
    }

    Ok(text.trim().to_string())
}

/// Extract text from PDF text operator (Tj or TJ)
#[cfg(feature = "pdf")]
fn extract_text_from_operator(line: &str) -> Option<String> {
    // Very basic extraction - looks for content between parentheses or angle brackets
    let line = line.trim();

    if line.contains("(") && line.contains(")") {
        // Text in parentheses: (text) Tj
        let start = line.find('(')?;
        let end = line.find(')')?;
        let text = &line[start + 1..end];
        return Some(decode_pdf_string(text));
    }

    if line.contains("<") && line.contains(">") {
        // Hexadecimal text: <hex> Tj
        let start = line.find('<')?;
        let end = line.find('>')?;
        let hex = &line[start + 1..end];
        return decode_pdf_hex(hex);
    }

    None
}

/// Decode PDF string (basic implementation)
#[cfg(feature = "pdf")]
fn decode_pdf_string(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\t", "\t")
        .replace("\\(", "(")
        .replace("\\)", ")")
        .replace("\\\\", "\\")
}

/// Decode PDF hexadecimal string
#[cfg(feature = "pdf")]
fn decode_pdf_hex(hex: &str) -> Option<String> {
    let hex_clean = hex.replace(" ", "");
    let bytes: Result<Vec<u8>, _> = (0..hex_clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_clean[i..i + 2], 16))
        .collect();

    bytes.ok().map(|b| String::from_utf8_lossy(&b).to_string())
}

/// Extract PDF metadata
#[cfg(feature = "pdf")]
fn extract_pdf_metadata(doc: &Document) -> PdfMetadata {
    let get_string_from_info = |key: &[u8]| -> Option<String> {
        doc.trailer
            .get(b"Info")
            .ok()
            .and_then(|obj| obj.as_reference().ok())
            .and_then(|(id1, id2)| doc.get_object((id1, id2)).ok())
            .and_then(|obj| obj.as_dict().ok())
            .and_then(|dict| dict.get(key).ok())
            .and_then(|obj| obj.as_str().ok())
            .map(|bytes| String::from_utf8_lossy(bytes).to_string())
    };

    PdfMetadata {
        title: get_string_from_info(b"Title"),
        author: get_string_from_info(b"Author"),
        subject: get_string_from_info(b"Subject"),
        creator: get_string_from_info(b"Creator"),
        producer: get_string_from_info(b"Producer"),
        creation_date: get_string_from_info(b"CreationDate"),
        modification_date: get_string_from_info(b"ModDate"),
    }
}

/// Fallback implementation when PDF feature is disabled
#[cfg(not(feature = "pdf"))]
pub fn extract_pdf_text(_pdf_bytes: &[u8], _max_pages: Option<usize>) -> Result<PdfInfo> {
    anyhow::bail!("PDF support is not enabled. Rebuild with --features pdf")
}

#[cfg(test)]
#[cfg(feature = "pdf")]
mod tests {
    use super::*;

    #[test]
    fn test_decode_pdf_string() {
        let input = "Hello\\nWorld\\(test\\)";
        let output = decode_pdf_string(input);
        assert_eq!(output, "Hello\nWorld(test)");
    }

    #[test]
    fn test_decode_pdf_hex() {
        let hex = "48656C6C6F"; // "Hello" in hex
        let output = decode_pdf_hex(hex);
        assert_eq!(output, Some("Hello".to_string()));
    }
}
