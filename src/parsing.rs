//! Handles parsing of a macro step.

use std::{
	collections::VecDeque,
	ops::Range,
	borrow::Cow
};

/// An object containing data about a macro match.
#[derive(PartialEq, Eq, Debug, Clone, Hash, Default)]
pub struct MacroRange<'source> {
	/// The range of the macro in the string.
	pub range: Range<usize>,
	/// The macro's name.
	pub name: &'source str,
	/// The macro's arguments.
	pub arguments: Vec<&'source str>
}

/// Tries to find the first macro pair in the string.
#[must_use]
pub fn find_pair(source: &str) -> Option<MacroRange<'_>> {
	let range = find_innermost_brackets(source)?;
	let inside = &source[range.start + 1 .. range.end - 1];
	let (name, arguments) = split_arguments(inside);
	Some(MacroRange { range, name, arguments })
}

/// Finds the first occurrence of an unescaped pair of square brackets.
#[allow(clippy::range_plus_one)]
fn find_innermost_brackets(string: &str) -> Option<Range<usize>> {
	// Find first [
	let mut last_escaped = false;
	let mut start = None;
	for (idx, chr) in string.char_indices() {
		if last_escaped {
			// Since this is escaped, we break out
			last_escaped = false;
			continue;
		}
		last_escaped = chr == '\\';
		match chr {
			'[' =>
				// We now have a potential starting point
				// We update this continually until we find a matching ]
				start = Some(idx),
			']' if start.is_some() => {
				// We've found a pair!
				let Some(start) = start else { unreachable!("must always be Some") }; // should be optimized out
				return Some(start .. idx + 1);
			},
			_ => {}
		}
	}
	None
}

/// Splits the inside of macro brackets into its name and arguments.
fn split_arguments(inside: &str) -> (&str, Vec<&str>) {
	let mut argument_spans = VecDeque::new();
	let mut last_escaped = false;
	let mut old_start = 0usize;
	for (idx, char) in inside.char_indices() {
		if last_escaped {
			last_escaped = false;
			continue;
		}
		last_escaped = char == '\\';
		if char == '/' {
			argument_spans.push_back(old_start .. idx);
			old_start = idx + 1;
		}
	}
	argument_spans.push_back(old_start .. inside.len());
	// This should be fine
	let name = argument_spans.pop_front().expect("we just pushed something");
	(&inside[name], argument_spans.into_iter().map(|range| &inside[range]).collect())
}

/// Unescapes a borrowed string, returning the borrow if they're the same.
pub(crate) fn unescape(original: &str) -> Cow<str> {
	let mut found_escape = false;
	let mut last_escape = false;
	let mut string = String::new();
	for (idx, char) in original.char_indices() {
		if !last_escape && char == '\\' {
		    if !found_escape {
		        string += &original[..idx];
		    }
			found_escape = true;
			last_escape = true;
			continue;
		}
		if !found_escape { continue }
		last_escape = false;
		string.push(char);
	}
	if found_escape {
		Cow::Owned(string)
	} else {
		Cow::Borrowed(original)
	}
}

#[cfg(test)]
mod test {
	use crate::parsing::*;

	#[test]
	fn bracket_test() {
		assert_eq!(find_innermost_brackets(r"[a[b[c[d]c][e]b]a]"), Some(6 .. 9));
		assert_eq!(find_innermost_brackets(r"\[[]\]"), Some(2 .. 4));
		assert_eq!(find_innermost_brackets(r"[[\][]"), Some(4 .. 6));
		assert_eq!(find_innermost_brackets(r"only open [[[ \]"), None);
		assert_eq!(find_innermost_brackets(r"[ no close \]\]"), None);
	}
}
