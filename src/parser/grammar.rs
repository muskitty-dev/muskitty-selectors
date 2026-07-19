//! Selectors grammar hooks for the §5.4.1 `parse_a_grammar` pipeline.
//!
//! Selectors Level 4 §18 L4828-4849 "Parse A Selector" specifies that
//! the source string is `[=CSS/parsed=]` as a `selector-list`. Per
//! CSS Syntax §5.4.1, `[=CSS/parsing=]` means: normalize → consume a
//! list of component values → match against the grammar. The two
//! [`Grammar`] implementors in this module perform that final
//! "match against the grammar" step for the selector-list and
//! relative-selector-list productions.
//!
//! # Architecture
//!
//! ```text
//!  parse_a_selector(source)
//!       │
//!       ▼
//!  muskitty_css::parser::parse_a_grammar(source, &SelectorGrammar)
//!       │
//!       │  §5.4.1 step 1-2: normalize → consume_a_list_of_component_values
//!       ▼
//!  SelectorGrammar::parse(&self, &[ComponentValue])
//!       │
//!       │  cv_adapter::cv_to_tokens → TokenStream
//!       ▼
//!  parser::list::parse_selector_list (existing §3 grammar)
//! ```
//!
//! # Why `Output = Result<SelectorList, SelectorParseError>`?
//!
//! The CSS Syntax [`Grammar`] trait's `parse` method returns
//! `Result<Self::Output, ParseError>` where `ParseError` is a marker
//! type with no payload (per §5.2 — the WHATWG algorithms themselves
//! don't carry diagnostic info). To preserve the Selectors-specific
//! error variants ([`SelectorParseError::EmptySelector`] vs
//! [`SelectorParseError::InvalidSelector`] vs etc.) we wrap the
//! Selectors-level result in the grammar's `Output`. The outer
//! `Result<_, ParseError>` only fires if the CSS Syntax pipeline
//! itself fails (which is impossible for normalised selector input).
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §18 L4828-4875;
//! `D:\CSSWG\css-syntax-3\Overview.md`, §5.4.1 L1895-1944.

use crate::error::SelectorParseError;
use crate::parser::cv_adapter::cv_to_tokens;
use crate::parser::{list, relative};
use crate::types::SelectorList;
use muskitty_css::parser::{ComponentValue, Grammar, ParseError, TokenStream};
use muskitty_css::tokenizer::Token;

/// §18 L4828-4849: Selector-list grammar for the §5.4.1 pipeline.
///
/// Implements [`Grammar`] with `Output = Result<SelectorList,
/// SelectorParseError>`. Apply via
/// [`muskitty_css::parser::parse_a_grammar`].
#[derive(Default, Debug, Clone, Copy)]
pub struct SelectorGrammar;

impl Grammar for SelectorGrammar {
    type Output = Result<SelectorList, SelectorParseError>;

    fn parse(&self, input: &[ComponentValue]) -> Result<Self::Output, ParseError> {
        let mut stream = build_token_stream(input);
        // §3 L1338: an empty selector is invalid. Pre-check so we can
        // return the more specific `EmptySelector` error variant
        // rather than the generic `InvalidSelector`.
        stream.discard_whitespace();
        if matches!(stream.next_token(), Token::Eof) {
            return Ok(Err(SelectorParseError::EmptySelector));
        }
        match list::parse_selector_list(&mut stream) {
            Ok(list) => {
                // §3 grammar: trailing tokens after the selector list
                // (other than whitespace) make the source invalid.
                stream.discard_whitespace();
                if !stream.is_empty() {
                    return Ok(Err(SelectorParseError::InvalidSelector(format!(
                        "trailing tokens after selector: {:?}",
                        stream.next_token()
                    ))));
                }
                Ok(Ok(list))
            }
            Err(e) => Ok(Err(e)),
        }
    }
}

/// §18 L4853-4875: Relative-selector-list grammar for the §5.4.1
/// pipeline.
///
/// Like [`SelectorGrammar`] but delegates to
/// [`crate::parser::relative::parse_relative_selector_list`]. Used by
/// `:has()` argument parsing.
#[derive(Default, Debug, Clone, Copy)]
pub struct RelativeSelectorGrammar;

impl Grammar for RelativeSelectorGrammar {
    type Output = Result<SelectorList, SelectorParseError>;

    fn parse(&self, input: &[ComponentValue]) -> Result<Self::Output, ParseError> {
        let mut stream = build_token_stream(input);
        stream.discard_whitespace();
        if matches!(stream.next_token(), Token::Eof) {
            return Ok(Err(SelectorParseError::EmptySelector));
        }
        match relative::parse_relative_selector_list(&mut stream) {
            Ok(list) => {
                stream.discard_whitespace();
                if !stream.is_empty() {
                    return Ok(Err(SelectorParseError::InvalidSelector(format!(
                        "trailing tokens after relative selector: {:?}",
                        stream.next_token()
                    ))));
                }
                Ok(Ok(list))
            }
            Err(e) => Ok(Err(e)),
        }
    }
}

/// Build a [`TokenStream`] from a slice of component values, adapting
/// them back to tokens via [`cv_adapter::cv_to_tokens`].
fn build_token_stream(cvs: &[ComponentValue]) -> TokenStream {
    let tokens = cv_to_tokens(cvs);
    TokenStream::new(tokens)
}
