use std::fmt;

use tree_sitter::{LanguageError, Parser, Tree};

pub struct BslParser {
    parser: Parser,
}

impl BslParser {
    pub fn new() -> Result<Self, BslParserError> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_bsl::LANGUAGE.into())
            .map_err(BslParserError::Language)?;
        Ok(Self { parser })
    }

    pub fn parse(&mut self, source: &str) -> Result<ParsedBsl, BslParserError> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or(BslParserError::ParseCancelled)?;
        Ok(ParsedBsl {
            tree,
            source_len: source.len(),
        })
    }
}

#[derive(Debug)]
pub struct ParsedBsl {
    tree: Tree,
    source_len: usize,
}

impl ParsedBsl {
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn source_len(&self) -> usize {
        self.source_len
    }

    pub fn has_errors(&self) -> bool {
        self.tree.root_node().has_error()
    }

    pub fn error_nodes(&self) -> Vec<BslParseErrorNode> {
        let mut errors = Vec::new();
        collect_error_nodes(self.tree.root_node(), &mut errors);
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BslParseErrorNode {
    pub kind: String,
    pub span: BslByteSpan,
    pub missing: bool,
}

impl BslParseErrorNode {
    fn from_node(node: tree_sitter::Node<'_>) -> Self {
        Self {
            kind: node.kind().to_owned(),
            span: BslByteSpan::new(node.start_byte(), node.end_byte()),
            missing: node.is_missing(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BslByteSpan {
    pub start_byte: usize,
    pub end_byte: usize,
}

impl BslByteSpan {
    pub fn new(start_byte: usize, end_byte: usize) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }
}

#[derive(Debug)]
pub enum BslParserError {
    Language(LanguageError),
    ParseCancelled,
}

impl fmt::Display for BslParserError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Language(source) => {
                write!(formatter, "failed to initialize BSL parser: {source}")
            }
            Self::ParseCancelled => write!(formatter, "BSL parser cancelled parsing"),
        }
    }
}

impl std::error::Error for BslParserError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Language(source) => Some(source),
            Self::ParseCancelled => None,
        }
    }
}

fn collect_error_nodes(node: tree_sitter::Node<'_>, errors: &mut Vec<BslParseErrorNode>) {
    if node.is_error() || node.is_missing() {
        errors.push(BslParseErrorNode::from_node(node));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() || child.is_error() || child.is_missing() {
            collect_error_nodes(child, errors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bsl_parser_initializes_shared_language() {
        BslParser::new().expect("BSL parser must initialize");
    }

    #[test]
    fn bsl_parser_parses_valid_utf8_module_without_errors() {
        let source = "Процедура Привет()\n    Сообщить(\"Привет\");\nКонецПроцедуры\n";
        let mut parser = BslParser::new().unwrap();

        let parsed = parser.parse(source).unwrap();

        assert_eq!(parsed.source_len(), source.len());
        assert!(!parsed.has_errors());
        assert!(parsed.error_nodes().is_empty());
        assert_eq!(parsed.tree().root_node().start_byte(), 0);
        assert_eq!(parsed.tree().root_node().end_byte(), source.len());
    }

    #[test]
    fn bsl_parser_preserves_utf8_byte_offsets_for_error_nodes() {
        let source = "Процедура Привет()\n    Если Тогда\nКонецПроцедуры\n";
        let mut parser = BslParser::new().unwrap();

        let parsed = parser.parse(source).unwrap();
        let errors = parsed.error_nodes();

        assert!(parsed.has_errors());
        assert!(!errors.is_empty());
        assert!(errors.iter().all(|error| {
            error.span.start_byte <= error.span.end_byte && error.span.end_byte <= source.len()
        }));
        assert!(
            errors
                .iter()
                .any(|error| error.span.start_byte > "Процедура ".len())
        );
    }

    #[test]
    fn bsl_parser_exposes_parse_errors_without_failing_parse() {
        let source = "Процедура Незавершенная()\n    Если Истина Тогда\n";
        let mut parser = BslParser::new().unwrap();

        let parsed = parser.parse(source).unwrap();

        assert!(parsed.has_errors());
        assert!(!parsed.error_nodes().is_empty());
    }
}
