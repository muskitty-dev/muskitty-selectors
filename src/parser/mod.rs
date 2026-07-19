//! Selectors Level 4 parser entry points.
//!
//! Implements the §18 "API Hooks" Parse A Selector / Parse A Relative
//! Selector algorithms by routing through the CSS Syntax §5.4.1
//! `parse_a_grammar` pipeline: normalize → consume a list of
//! component values → match against the Selectors grammar via
//! [`SelectorGrammar`] / [`RelativeSelectorGrammar`].
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §18 L4816-5026
//! (API hooks), §3 L4640-4815 (grammar); `D:\CSSWG\css-syntax-3\Overview.md`,
//! §5.4.1 L1895-1944 (parse something according to a CSS grammar).

pub mod an_plus_b;
pub mod complex;
pub mod compound;
pub mod cv_adapter;
pub mod grammar;
pub mod list;
pub mod relative;
pub mod simple;

pub use grammar::{RelativeSelectorGrammar, SelectorGrammar};

use crate::error::SelectorParseError;
use crate::types::SelectorList;
use muskitty_css::parser::parse_a_grammar;

/// §18 L4828-4849: Parse A Selector.
///
/// Routes `source` through the CSS Syntax §5.4.1 pipeline (normalize
/// → consume a list of component values → match against
/// [`SelectorGrammar`]), per §18 L4837: "Let selector be the result of
/// [=CSS/parsing=] source as a `selector-list`."
///
/// Returns the parsed [`SelectorList`] on success, or a
/// [`SelectorParseError`] describing the failure mode.
///
/// Empty input or whitespace-only input returns
/// [`SelectorParseError::EmptySelector`] per §3 L1338 ("an empty
/// selector is invalid"). Trailing tokens after the selector list
/// (other than whitespace) produce an `InvalidSelector` error: a
/// selector source must consume the entire input.
pub fn parse_a_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    match parse_a_grammar(source, &SelectorGrammar) {
        Ok(inner) => inner,
        Err(_) => Err(SelectorParseError::InvalidSelector(
            "css-syntax §5.4.1 pipeline failure".into(),
        )),
    }
}

/// §18 L4853-4875: Parse A Relative Selector.
///
/// Like [`parse_a_selector`] but the source is interpreted as a
/// relative selector (relative to an implicit `:scope` element, per
/// §3 L1051-1102). Used by `:has()` arguments.
///
/// Routes through the §5.4.1 pipeline with
/// [`RelativeSelectorGrammar`], per §18 L4862: "Let selector be the
/// result of [=CSS/parsing=] source as a `relative-selector-list`."
pub fn parse_a_relative_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    match parse_a_grammar(source, &RelativeSelectorGrammar) {
        Ok(inner) => inner,
        Err(_) => Err(SelectorParseError::InvalidSelector(
            "css-syntax §5.4.1 pipeline failure".into(),
        )),
    }
}
