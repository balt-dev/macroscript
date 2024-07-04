/*!
Handles everything relating to text-based macros.

If you want help writing a text macro, see the documentation of [`TextMacro`].
*/

use std::{
	ops::Range,
	borrow::Cow,
	cell::LazyCell,
	str::FromStr
};
use crate::{Macro, MacroError};

/**
Simplifies creating macros by allowing you to compose them from other macros.

Text macros output their definition with certain argument strings replaced by the macro's arguments.
The following strings are replaced:
- `$#` Replaced with the amount of arguments.
- `$0` Replaced with all arguments separated by `/`.
- `$<num>` Replaced with the argument at the given index (one-based). The argument is not replaced if it doesn't exist.

The strings are replaced from back to front, and if another one is constructed while replacing them, it will be replaced as well.

## Example
```
#    use macroscript::{Macro, apply_macros, TextMacro, add_stdlib};
#    use std::collections::HashMap;
#    
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut macros = HashMap::<String, Box<dyn Macro>>::from([
    ("bad_select".to_string(), TextMacro::boxed("$$1")),
    ("escaped_dollar".to_string(), TextMacro::boxed(r"\$1")),
	("square".to_string(), TextMacro::boxed("[multiply/$1/$1]"))
]);
add_stdlib(&mut macros);
assert_eq!("$1", apply_macros("[escaped_dollar/2]".into(), &macros)?);
assert_eq!("α", apply_macros("[bad_select/2/α]".into(), &macros)?);
assert_eq!("$3", apply_macros("[bad_select/3]".into(), &macros)?);
assert_eq!("0/1/2/3", apply_macros("[bad_select/0/1/2/3]".into(), &macros)?);
assert_eq!("4", apply_macros("[bad_select/#/β/2/3]".into(), &macros)?);
assert_eq!("16", apply_macros("[square/4]".into(), &macros)?);
#        Ok(()) }
```

## Implementation Detail
The character `\u{FFFF}` is used to replace escaped `$` where needed.
This means any instances of those bytes in the string will be replaced with `$`.
*/
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct TextMacro {
	/// The pattern of the text macro.
	pub pattern: String
}

impl TextMacro {
	/// Creates a new text macro.
	#[inline]
	pub fn new(pattern: impl Into<String>) -> Self {
		Self { pattern: pattern.into() }
	}

	/// Creates a new text macro in a box. Mostly useful for directly adding to a [`std::collections::HashMap`].
	#[inline]
	pub fn boxed(pattern: impl Into<String>) -> Box<dyn Macro> {
		Box::new(Self { pattern: pattern.into() })
	}
}

impl From<String> for TextMacro {
	fn from(pattern: String) -> Self {
		Self {pattern}
	}
}

impl From<TextMacro> for String {
	fn from(mac: TextMacro) -> String {
		mac.pattern
	}
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Substring {
	Count,
	Index(usize)
}


// In the Python version, a regular expression with a negative lookbehind with a backslash was used.
// Unfortunately, Rust's regex library doesn't support lookarounds.
// I've reimplemented this without regex entirely.
impl Macro for TextMacro {
	fn apply(&self, _range: Range<usize>, arguments: Vec<&str>) -> Result<String, MacroError> {
		let amount = LazyCell::new(|| arguments.len().to_string());
		let joined = LazyCell::new(|| arguments.join("/"));
		let mut target = self.pattern.clone();
		// Find instance of $# or $\d+, but not \$, from back
		// We use an auxiliary character to weed out the `\$`s.
		target = target.replace(r"\$", "\u{FFFF}");
		let mut none_found = false;
		let mut subs = Vec::new();
		while !none_found {
			none_found = true;
			subs.clear();
			let mut passed = target.clone();
			for (idx, _) in passed.rmatch_indices('$') {
				let after_index = &passed[idx+1..];
				let (sub, end) = match after_index.chars().next() {
					Some('#') => (Substring::Count, idx + 2),
					Some('0') => (Substring::Index(0), idx + 2),
					Some(c) if c.is_ascii_digit()  => {
						let end = after_index.find(|c: char| !c.is_ascii_digit()).unwrap_or(after_index.len());
						let Some(index) = usize::from_str(&after_index[..end])
							.ok()
							.filter(|v| arguments.len() >= *v)
						else { continue };
						(Substring::Index(index), end + 1)
					},
					_ => { continue }
				};
				subs.push((idx .. idx + end, sub));
			}
			for (range, substring) in subs.drain(..) {
				let repl: Cow<'_, str> = match substring {
					Substring::Count => Cow::Borrowed(&*amount),
					Substring::Index(0) => Cow::Borrowed(&*joined),
					Substring::Index(n) => if let Some(arg) = arguments.get(n-1) {
						Cow::Borrowed(&**arg)
					} else { continue }
				};
				none_found = false;
				dbg!(&passed, &range, &repl);
				passed.replace_range(range, &repl);
			}
			target = passed;
		}
		target = target.replace('\u{FFFF}', "$");
		Ok(target)
	}
}
