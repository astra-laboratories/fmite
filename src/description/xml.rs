//! An indenting XML writer and `xs:double` text for floats: all the XML a model
//! description needs, with no knowledge of FMI.

use core::fmt::Write as _;

/// An indenting XML writer.
pub struct Xml {
    out: String,
    depth: usize,
}

impl Xml {
    /// A UTF-8 document with its declaration written.
    #[must_use]
    pub fn document() -> Self {
        Self {
            out: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n".to_owned(),
            depth: 0,
        }
    }

    /// The text written so far.
    #[must_use]
    pub fn finish(self) -> String {
        self.out
    }

    fn start(&mut self, name: &str, attributes: &[(&str, String)]) {
        let indent = "  ".repeat(self.depth);
        let _ = write!(self.out, "{indent}<{name}");
        for (key, value) in attributes {
            let _ = write!(self.out, " {key}=\"{}\"", escape(value));
        }
    }

    pub fn open(&mut self, name: &str, attributes: &[(&str, String)]) {
        self.start(name, attributes);
        self.out.push_str(">\n");
        self.depth += 1;
    }

    pub fn empty(&mut self, name: &str, attributes: &[(&str, String)]) {
        self.start(name, attributes);
        self.out.push_str("/>\n");
    }

    pub fn close(&mut self, name: &str) {
        self.depth -= 1;
        let indent = "  ".repeat(self.depth);
        let _ = writeln!(self.out, "{indent}</{name}>");
    }
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            c => escaped.push(c),
        }
    }
    escaped
}

/// A float as `xs:double` writes it: the shortest text that reads back exactly.
pub fn number(x: f64) -> String {
    if x.is_nan() {
        "NaN".to_owned()
    } else if x.is_infinite() {
        if x > 0.0 { "INF" } else { "-INF" }.to_owned()
    } else {
        x.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_text_is_escaped() {
        assert_eq!(escape(r#"a < b & "c""#), "a &lt; b &amp; &quot;c&quot;");
    }

    #[test]
    fn floats_are_written_as_xs_double() {
        assert_eq!(number(0.1), "0.1");
        assert_eq!(number(273.15), "273.15");
        assert_eq!(number(f64::INFINITY), "INF");
        assert_eq!(number(f64::NEG_INFINITY), "-INF");
        assert_eq!(number(f64::NAN), "NaN");
    }
}
