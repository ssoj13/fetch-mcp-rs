use anyhow::Result;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Selected HTML element with text and attributes
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ElementData {
    /// Element text content
    pub text: String,

    /// Element HTML (outer HTML)
    pub html: Option<String>,

    /// Element attributes (key-value pairs)
    pub attributes: Vec<(String, String)>,
}

/// Table data structure
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TableData {
    /// Table headers (if present)
    pub headers: Vec<String>,

    /// Table rows (each row is an array of cells)
    pub rows: Vec<Vec<String>>,

    /// Number of columns
    pub columns: usize,

    /// Number of rows
    pub row_count: usize,
}

/// Select elements from HTML using CSS selector
pub fn select_elements(html: &str, css_selector: &str) -> Result<Vec<ElementData>> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(css_selector)
        .map_err(|e| anyhow::anyhow!("Invalid CSS selector: {:?}", e))?;

    let elements: Vec<ElementData> = document
        .select(&selector)
        .map(|element| {
            let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let html_content = element.html();

            let attributes: Vec<(String, String)> = element
                .value()
                .attrs()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect();

            ElementData {
                text,
                html: Some(html_content),
                attributes,
            }
        })
        .collect();

    Ok(elements)
}

/// Extract table data from HTML
/// If selector is provided, extracts the first matching table
/// Otherwise, extracts the first table found
pub fn extract_table(html: &str, table_selector: Option<&str>) -> Result<Vec<TableData>> {
    let document = Html::parse_document(html);

    let selector_str = table_selector.unwrap_or("table");
    let table_sel = Selector::parse(selector_str)
        .map_err(|e| anyhow::anyhow!("Invalid table selector: {:?}", e))?;

    let thead_sel = Selector::parse("thead tr th, thead tr td")
        .map_err(|e| anyhow::anyhow!("Failed to create thead selector: {:?}", e))?;
    let tbody_sel = Selector::parse("tbody tr")
        .map_err(|e| anyhow::anyhow!("Failed to create tbody selector: {:?}", e))?;
    let cell_sel = Selector::parse("td, th")
        .map_err(|e| anyhow::anyhow!("Failed to create cell selector: {:?}", e))?;

    let mut tables = Vec::new();

    for table_element in document.select(&table_sel) {
        // Extract headers
        let headers: Vec<String> = table_element
            .select(&thead_sel)
            .map(|cell| cell.text().collect::<Vec<_>>().join(" ").trim().to_string())
            .collect();

        // Extract rows
        let rows: Vec<Vec<String>> = table_element
            .select(&tbody_sel)
            .filter(|row| {
                // Skip header rows in tbody
                !row.value().name().eq_ignore_ascii_case("th")
            })
            .map(|row| {
                row.select(&cell_sel)
                    .map(|cell| cell.text().collect::<Vec<_>>().join(" ").trim().to_string())
                    .collect()
            })
            .filter(|row: &Vec<String>| !row.is_empty())
            .collect();

        let columns = rows.iter().map(|r| r.len()).max().unwrap_or(headers.len());
        let row_count = rows.len();

        tables.push(TableData {
            headers,
            rows,
            columns,
            row_count,
        });
    }

    if tables.is_empty() {
        anyhow::bail!("No tables found with selector '{}'", selector_str);
    }

    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_elements() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div class="item">Item 1</div>
                <div class="item">Item 2</div>
                <a href="https://example.com" class="link">Link</a>
            </body>
            </html>
        "#;

        let result = select_elements(html, ".item");
        assert!(result.is_ok());

        let elements = result.unwrap();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].text, "Item 1");
        assert_eq!(elements[1].text, "Item 2");
    }

    #[test]
    fn test_select_elements_with_attributes() {
        let html = r#"<a href="https://example.com" class="link">Link</a>"#;

        let result = select_elements(html, "a");
        assert!(result.is_ok());

        let elements = result.unwrap();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].text, "Link");

        let href_attr = elements[0]
            .attributes
            .iter()
            .find(|(k, _)| k == "href");
        assert!(href_attr.is_some());
        assert_eq!(href_attr.unwrap().1, "https://example.com");
    }

    #[test]
    fn test_extract_table() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <table>
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Age</th>
                            <th>City</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>John</td>
                            <td>30</td>
                            <td>New York</td>
                        </tr>
                        <tr>
                            <td>Jane</td>
                            <td>25</td>
                            <td>London</td>
                        </tr>
                    </tbody>
                </table>
            </body>
            </html>
        "#;

        let result = extract_table(html, None);
        assert!(result.is_ok());

        let tables = result.unwrap();
        assert_eq!(tables.len(), 1);

        let table = &tables[0];
        assert_eq!(table.headers, vec!["Name", "Age", "City"]);
        assert_eq!(table.row_count, 2);
        assert_eq!(table.columns, 3);
        assert_eq!(table.rows[0], vec!["John", "30", "New York"]);
        assert_eq!(table.rows[1], vec!["Jane", "25", "London"]);
    }

    #[test]
    fn test_extract_table_no_thead() {
        let html = r#"
            <table>
                <tr><td>A</td><td>B</td></tr>
                <tr><td>1</td><td>2</td></tr>
            </table>
        "#;

        let result = extract_table(html, None);
        assert!(result.is_ok());

        let tables = result.unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].row_count, 2);
    }
}
