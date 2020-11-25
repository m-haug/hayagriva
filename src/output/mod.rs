//! Citation and bibliography styles.

use super::types::Person;
use super::Entry;
use std::collections::HashMap;
use std::convert::Into;
use std::ops::{Add, AddAssign};
use thiserror::Error;

pub mod apa;
pub mod chicago;
pub mod ieee;
pub mod mla;

/// This enum describes where the output string of the [CitationFormatter]
/// should be set: Inside the text or in a
pub enum CitationMode {
    /// Set citation text in a footnote. Only produce a superscript footnote
    /// symbol at the matching position of the text. (footnote numbers are
    /// managed by the callee; may be rendered as endnotes).
    Footnote,
    /// The citation text should be set directly where the text appears.
    InText,
}

/// Will be raised if a user-specified citation is not possible with the given
/// database.
#[derive(Debug, Error)]
pub enum CitationError {
    /// A key could not be found.
    #[error("key {0} could not be fount in the citation database")]
    KeyNotFound(String),
    /// A number was required for this citation format but not found.
    #[error("key {0} did not contain a number")]
    NoNumber(String),
}

/// Structs that implement this can be used to generate bibliography references
/// for sources.
pub trait BibliographyFormatter {
    /// Get a string with optional formatting that describes `Entry` in
    /// accordance with the implementing struct's style.
    fn get_reference(&self, entry: &Entry, prev_entry: Option<&Entry>) -> DisplayString;
}

/// Represents a citation of one or more database entries.
#[derive(Clone, Debug)]
pub struct AtomicCitation<'s> {
    /// Cited entry keys.
    pub key: &'s str,
    /// Supplements for each entry key such as page or chapter number.
    pub supplement: Option<&'s str>,
    /// Assigned number of the citation.
    pub number: Option<usize>,
}

/// Structs implementing this trait can generate the appropriate reference
/// markers for a single `Citation` struct. They do not have to see subsequent
/// citations to determine the marker value.
pub trait CitationFormatter<'s> {
    /// Get a reference for the passed citation struct.
    fn get_reference(
        &self,
        citation: impl Iterator<Item = AtomicCitation<'s>>,
    ) -> Result<String, CitationError>;
}

/// Checks if the keys are in the database and returns them as reference
/// markers, since they are already unique.
pub struct KeyCitationFormatter<'s> {
    entries: &'s HashMap<String, Entry>,
}

impl<'s> CitationFormatter<'s> for KeyCitationFormatter<'s> {
    fn get_reference(
        &self,
        citation: impl Iterator<Item = AtomicCitation<'s>>,
    ) -> Result<String, CitationError> {
        let mut items = vec![];
        for atomic in citation {
            if !self.entries.contains_key(atomic.key) {
                return Err(CitationError::KeyNotFound(atomic.key.to_string()));
            }

            items.push(if let Some(supplement) = atomic.supplement {
                format!("{} ({})", atomic.key, supplement)
            } else {
                atomic.key.to_string()
            });
        }

        Ok(items.join(", "))
    }
}

/// Output IEEE-style numerical reference markers.
pub struct NumericalCitationFormatter<'s> {
    entries: &'s HashMap<String, Entry>,
}

impl<'s> CitationFormatter<'s> for NumericalCitationFormatter<'s> {
    fn get_reference(
        &self,
        citation: impl Iterator<Item = AtomicCitation<'s>>,
    ) -> Result<String, CitationError> {
        let mut ids = vec![];
        for atomic in citation {
            if !self.entries.contains_key(atomic.key) {
                return Err(CitationError::KeyNotFound(atomic.key.to_string()));
            }

            let number = atomic
                .number
                .ok_or_else(|| CitationError::NoNumber(atomic.key.to_string()))?;
            ids.push((number, atomic.supplement));
        }

        ids.sort_by(|(a, _), (b, _)| a.cmp(&b));

        enum CiteElement<'a> {
            Range(std::ops::Range<usize>),
            Single((usize, Option<&'a str>)),
        }

        let mut res_elems = vec![];

        for (number, supplement) in ids {
            if let Some(s) = supplement {
                res_elems.push(CiteElement::Single((number, Some(s))));
                continue;
            }

            match res_elems.last() {
                Some(CiteElement::Range(r)) if r.end == number - 1 => {
                    let mut r = r.clone();
                    res_elems.pop().unwrap();
                    r.end = number;
                    res_elems.push(CiteElement::Range(r));
                }
                _ if supplement.is_some() => {
                    res_elems.push(CiteElement::Single((number, supplement)));
                }
                _ => {
                    res_elems.push(CiteElement::Range(number .. number));
                }
            }
        }

        let re = res_elems
            .into_iter()
            .map(|e| match e {
                CiteElement::Range(r) if r.start != r.end => {
                    format!("{}-{}", r.start, r.end)
                }
                CiteElement::Range(r) => r.start.to_string(),
                CiteElement::Single((n, s)) => {
                    if let Some(sup) = s {
                        format!("{}, {}", n, sup)
                    } else {
                        n.to_string()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join("; ");

        Ok(format!("[{}]", re))
    }
}

fn format_range<T: std::fmt::Display + PartialEq>(
    prefix_s: &str,
    prefix_m: &str,
    range: &std::ops::Range<T>,
) -> String {
    let space = if prefix_s.is_empty() { "" } else { " " };
    if range.start == range.end {
        format!("{}{}{}", prefix_s, space, range.start)
    } else {
        format!("{}{}{}–{}", prefix_m, space, range.start, range.end)
    }
}

fn name_list(persons: &[Person]) -> Vec<String> {
    let mut names = vec![];

    for author in persons.iter() {
        names.push(author.get_name_first(true, false));
    }

    names
}

fn name_list_straight(persons: &[Person]) -> Vec<String> {
    let mut names = vec![];

    for author in persons.iter() {
        names.push(author.get_given_name_initials_first(true));
    }

    names
}

/// Formatting modifiers for strings.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Formatting {
    /// **Bold print**
    Bold,
    /// _italic print_
    Italic,
    /// Do not hyphenate, e.g. for URLs.
    NoHyphenation,
}

/// Will move a format range's indicies by `o`.
pub(crate) fn offset_format_range(
    r: (std::ops::Range<usize>, Formatting),
    o: usize,
) -> (std::ops::Range<usize>, Formatting) {
    ((r.0.start + o) .. (r.0.end + o), r.1)
}

/// A printable string with a list of formatting modifications
#[derive(Clone, Debug)]
pub struct DisplayString {
    /// The string content.
    pub value: String,
    /// Information about formatted ranges.
    pub formatting: Vec<(std::ops::Range<usize>, Formatting)>,

    pending_formatting: Vec<(usize, Formatting)>,
}

impl Add<&str> for DisplayString {
    type Output = DisplayString;

    #[inline]
    fn add(mut self, other: &str) -> DisplayString {
        self.value.push_str(other);
        self
    }
}

impl Add<Self> for DisplayString {
    type Output = Self;

    #[inline]
    fn add(mut self, other: Self) -> Self {
        self.formatting.append(
            &mut other
                .formatting
                .into_iter()
                .map(|e| offset_format_range(e, self.value.len()))
                .collect(),
        );
        self.value.push_str(&other.value);
        self
    }
}

impl AddAssign<&String> for DisplayString {
    fn add_assign(&mut self, other: &String) {
        self.value.push_str(other);
    }
}

impl AddAssign<&str> for DisplayString {
    fn add_assign(&mut self, other: &str) {
        self.value.push_str(other);
    }
}

impl AddAssign<Self> for DisplayString {
    fn add_assign(&mut self, other: Self) {
        self.formatting.append(
            &mut other
                .formatting
                .into_iter()
                .map(|e| offset_format_range(e, self.value.len()))
                .collect(),
        );
        self.value.push_str(&other.value);
    }
}

impl Into<String> for DisplayString {
    fn into(self) -> String {
        self.value
    }
}

impl Into<DisplayString> for String {
    fn into(self) -> DisplayString {
        DisplayString::from_string(self)
    }
}

impl Into<DisplayString> for &str {
    fn into(self) -> DisplayString {
        DisplayString::from_str(self)
    }
}

impl DisplayString {
    /// Constructs a new DisplayString.
    pub fn new() -> Self {
        Self {
            value: String::new(),
            formatting: vec![],
            pending_formatting: vec![],
        }
    }

    /// Uses a string reference to create a display string.
    pub fn from_str(s: &str) -> Self {
        Self {
            value: s.to_string(),
            formatting: vec![],
            pending_formatting: vec![],
        }
    }

    /// Use a String to create a display string.
    pub fn from_string(s: String) -> Self {
        Self {
            value: s,
            formatting: vec![],
            pending_formatting: vec![],
        }
    }

    /// Get the length of the string.
    pub fn len(&self) -> usize {
        self.value.len()
    }

    /// Is the string empty?
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Get the last character.
    pub fn last(&self) -> Option<char> {
        self.value.chars().last()
    }

    /// Push onto the string.
    pub fn push(&mut self, ch: char) {
        self.value.push(ch);
    }

    pub(crate) fn start_format(&mut self, f: Formatting) {
        debug_assert!(!self.pending_formatting.iter().any(|e| e.1 == f));
        self.pending_formatting.push((self.len(), f));
    }

    pub(crate) fn commit_formats(&mut self) {
        for (start, f) in self.pending_formatting.iter() {
            self.formatting.push((*start .. self.len(), *f))
        }

        self.pending_formatting.clear();
    }

    pub(crate) fn add_if_some<S: Into<String>>(
        &mut self,
        item: Option<S>,
        prefix: Option<&str>,
        postfix: Option<&str>,
    ) {
        if let Some(item) = item {
            if let Some(prefix) = prefix {
                *self += prefix;
            }
            *self += &item.into();
            if let Some(postfix) = postfix {
                *self += postfix;
            }
        }
    }

    /// Joins a number of display strings with a seperator in-between.
    pub fn join(items: &[Self], joiner: &str) -> Self {
        let mut res = DisplayString::new();
        for (i, e) in items.iter().enumerate() {
            if i != 0 {
                res += joiner;
            }

            res += e.clone();
        }

        res
    }

    /// Applies the formatting as ANSI / VT100 control sequences and
    /// prints that formatted string to standard output.
    pub fn print_ansi_vt100(&self) -> String {
        let mut start_end = vec![];

        for item in &self.formatting {
            let opt = item.1;
            if opt == Formatting::NoHyphenation {
                continue;
            }
            let min = item.0.start;
            let max = item.0.end;

            start_end.push((opt.clone(), min, false));
            start_end.push((opt, max, true));
        }

        start_end.sort_by(|a, b| a.1.cmp(&b.1).reverse());

        let mut res = String::new();
        let mut pointer = self.len();

        for (f, index, end) in &start_end {
            res = (&self.value[*index .. pointer]).to_string() + &res;
            pointer = *index;

            let code = if *end {
                "0"
            } else {
                match f {
                    Formatting::Bold => "1",
                    Formatting::Italic => "3",
                    Formatting::NoHyphenation => unreachable!(),
                }
            };
            res = format!("\x1b[{}m", code) + &res;
        }
        res = (&self.value[0 .. pointer]).to_string() + &res;

        res
    }
}
