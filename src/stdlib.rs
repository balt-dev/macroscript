use crate::execution::{Macro, MacroError, MacroErrorKind};
use itertools::Itertools;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::str::FromStr;
use std::hash::{Hasher, BuildHasher};
use rand_pcg::Pcg32;
use rand::{self, Rng, SeedableRng};
use seahash::SeaHasher;
use regex::Regex;

macro_rules! count {
	($tt: tt $($tts: tt)*) => {
		1 + count!($($tts)*)
	};
	() => {0}
}

macro_rules! builtin_macros {
	($($(#[$attr: meta])* macro $id: ident as $name: literal {$inner: item})*) => {$(
		#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, Hash)]
		$(#[$attr])*
		pub struct $id;
		
		impl Macro for $id {
			$inner
		}
	)*

		/// Adds the standard library's builtin macros to a map of macro names.
		pub fn add_stdlib(macros: &mut HashMap<String, &dyn Macro, impl BuildHasher>) {
		    $(
		    	macros.insert($name.into(), &$id);
		    )*
		}
	}
}

macro_rules! get_args {
	($name: literal, $range: ident, $arguments: ident; $($ids: ident),+) => {{
		let mut args = $arguments.iter();
		let c = count!($($ids)*);
		get_args!{_recur $name $range args; c $($ids)* | }
	}};
	(_recur $name: literal $range: ident $args: ident; $amount: ident $id: ident $($ids: ident)* | $($leftover: ident)*) => {
		let Some($id) = $args.next() else {
	  		return Err(MacroError::new(
	  			$name.into(), $range,
	  			MacroErrorKind::not_enough_args($amount, count!($($leftover)*))
	  		));
	  	};
		get_args!{ _recur $name $range $args; $amount $($ids)* | $($leftover)* $id }
	};
	(_recur $name: literal $range: ident $args: ident; $amount: ident | $($leftover: ident)*) => {
		($($leftover,)*)
	}
}

macro_rules! convert_to_number {
	($name: literal, $range: expr; at $idx: expr => $arg: expr) => {{
		let arg = $arg;
		f64::from_str(arg).map_err(|_| {
            MacroError::new(
            	$name.into(), $range.clone(),
            	MacroErrorKind::user(
            		format!("could not convert argument {} \"{arg}\" to number", $idx)
           		)
            )
        })?
	}}
}

fn truthy(string: impl AsRef<str>) -> bool {
	match string.as_ref() {
		"true" | "True" => true,
		v if f64::from_str(v).is_ok_and(|v| v != 0. && !v.is_nan()) => true,
		_ => false
	}
}

builtin_macros! {
	/// Addition. Takes 0 or more numeric arguments and returns their sum.
	/// # Examples
	/// ```ignore
	///	[add/3/2/3/5/3] // 16
	/// [add/5] // 5
	/// [add] // 0
	/// [add/a/b] // error: could not convert argument 1 to number
	/// ```
	macro BuiltinAdd as "add" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
		    arguments
	            .iter()
	            .enumerate()
	            .map(|(idx, arg)| {
	                Ok(convert_to_number!("add", range; at idx+1 => arg))
	            })
	            .process_results(|iter| iter.sum())
	            .map(|sum: f64| sum.to_string())
		}
	}

	/// Multiplicaton. Takes 0 or more numeric arguments and returns their product.
	/// # Examples
	/// ```ignore
	///	[multiply/1/2/3/4/5] // 120
	/// [multiply/5] // 5
	/// [multiply] // 1
	/// [multiply/a/b] // error: could not convert argument 1 to number
	/// ```
	macro BuiltinMultiply as "multiply" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
		    arguments
	            .iter()
	            .enumerate()
	            .map(|(idx, arg)| {
	                Ok(convert_to_number!("multiply", range; at idx+1 => arg))
	            })
	            .process_results(|iter| iter.product())
	            .map(|product: f64| product.to_string())
		}
	}

	/// Unescapes its input. This can be used to lazily evaluate a macro.
	/// # Examples
	/// ```ignore
	/// [unescape/among\[us\]] // among[us]
	/// [unescape/[if/true/\[a\]/\[b\]]] // [a]
	/// [unescape/[if/false/\[a\]/\[b\]]] // [b]
	/// ```
	macro BuiltinUnescape as "unescape" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (first_arg, ) = get_args!("unescape", range, arguments; first_arg);
       		let unescaped = crate::parsing::unescape(first_arg);
        	Ok(unescaped.into_owned())
		}
	}

	/// Basic alternation. Chooses between all even arguments with the condition of the odd ones,
	///  with the last as a base case.
	/// # Examples
	/// ```ignore
	/// [if/true/a/true/b/c] // a
	/// [if/false/a/true/b/c] // b
	/// [if/false/a/false/b/c] // c
	/// [if/true/a/true/b] // errors
	/// [if/c] // c
	/// ```
	macro BuiltinIf as "if" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
			let mut chunks = arguments.chunks_exact(2);
			// Technically refutable pattern
			while let Some([condition, value]) = chunks.next() {
				if truthy(condition) {
					return Ok(value.to_string());
				}
			}
			if let [end] = chunks.remainder() {
				Ok(end.to_string())
			} else {
				Err(MacroError {
					name: "if".into(),
					error_type: MacroErrorKind::User {
						message: "all conditions exhausted without base case".into()
					},
					range
				})
			}
		}
	}

	/*
	macro BuiltinName as "name" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
			
		}
	}
	*/

	/// Returns whether a string can be converted to a number.
	/// # Examples
	/// ```ignore
	/// [is_number/1] // true
	/// [is_number/abc] // false
	/// ```
	macro BuiltinIsNumber as "is_number" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (first_arg, ) = get_args!("is_number", range, arguments; first_arg);
 		  	Ok(f64::from_str(first_arg).is_ok().to_string())
		}
	}

	/// Raises a number to the power of another.
	/// # Examples
	/// ```ignore
	/// [pow/7/2] // 49.0
	/// ``` 
	macro BuiltinPow as "pow" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (base, exp) = get_args!("pow", range, arguments; base, exp);
 		  	let base = convert_to_number!("pow", range; at 1 => base);
 		  	let exp = convert_to_number!("pow", range; at 2 => exp);
			Ok(base.powf(exp).to_string())
		}
	}

	/// Subtracts a number from another.
	/// # Examples
	/// ```ignore
	/// [subtract/7/2] // 5.0
	/// [subtract/3/5] // -2.0
	/// ``` 
	macro BuiltinSub as "subtract" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (lhs, rhs) = get_args!("subtract", range, arguments; a, b);
 		  	let lhs = convert_to_number!("subtract", range; at 1 => lhs);
 		  	let rhs = convert_to_number!("subtract", range; at 2 => rhs);
			Ok((lhs - rhs).to_string())
		}
	}
	
	/// Divides a number by another.
	/// # Examples
	/// ```ignore
	/// [divide/5/2] // 2.5 
	/// [divide/3/5] // 0.6
	/// [divide/1/0] // inf
	/// [divide/-1/0] // -inf
	/// [divide/0/0] // nan
	/// ``` 
	macro BuiltinDiv as "divide" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (lhs, rhs) = get_args!("divide", range, arguments; a, b);
 		  	let lhs = convert_to_number!("divide", range; at 1 => lhs);
 		  	let rhs = convert_to_number!("divide", range; at 2 => rhs);
			Ok((lhs / rhs).to_string())
		}
	}
	
	/// Takes the modulus of one number with respect to another.
	/// # Examples
	/// ```ignore
	/// [mod/5/2] // 1.0
	/// [mod/-3/5] // 2.0
	/// ``` 
	macro BuiltinModulus as "mod" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (lhs, rhs) = get_args!("mod", range, arguments; a, b);
 		  	let lhs = convert_to_number!("mod", range; at 1 => lhs);
 		  	let rhs = convert_to_number!("mod", range; at 2 => rhs);
			Ok((lhs.rem_euclid(rhs)).to_string())
		}
	}

	
	/// Takes the logarithm of a number. The base is optional, and defaults to [`f64::E`].
	/// # Examples
	/// ```ignore
	/// [log/5] // 1.6094
	/// [log/16/2] // 4.0
	/// ``` 
	macro BuiltinLog as "log" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (value, ) = get_args!("log", range, arguments; value);
 		  	let value = convert_to_number!("log", range; at 1 => value);
 		  	let base = if let Some(base) = arguments.get(1) {
 		  		convert_to_number!("log", range; at 2 => base)
 		  	} else {
 		  		std::f64::consts::E
 		  	};
			Ok(value.log(base).to_string())
		}
	}

	/// Gets a random number on the range [0, 1).
	/// A seed can optionally be supplied.
	/// # Examples
	/// ```ignore
	/// [rand] // ?
	/// [rand/among us] // 0.226
	/// ``` 
	macro BuiltinRand as "rand" {
		fn apply(&self, _range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let value: f64 = if let Some(seed) = arguments.first() {
 		  		let mut hasher = SeaHasher::new();
 		  		hasher.write(seed.as_bytes());
				let mut rand = Pcg32::seed_from_u64(hasher.finish());
				rand.gen()
 		  	} else {
 		  		rand::random()
 		  	};
 		  	Ok(value.to_string())
		}
	}

	/// Hashes a value, returning a 64-bit integer.
	/// # Examples
	/// ```ignore
	/// [hash/rain world] // 13463560454117874234
	/// ```
	macro BuiltinHash as "hash" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (value, ) = get_args!("hash", range, arguments; a);
 		  	let mut hasher = SeaHasher::new();
 		  	hasher.write(value.as_bytes());
 		  	Ok(hasher.finish().to_string())			
		}
	}

	/// Replaces all matches of a regular expression with a pattern.
	/// The regular expression and replacement pattern are unescaped before use.
	/// # Examples
	/// ```ignore
	/// [replace/vaporeon/\[aeiou\]/$1$1] // vaapooreeoon
	/// [replace/porygon/\[o/e] // error: invalid regex
	/// ```
	macro BuiltinReplace as "replace" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (haystack, pattern, replacement) = get_args!("hash", range, arguments; a, b, c);
 		  	let pattern = crate::parsing::unescape(pattern);
 		  	let replacement = crate::parsing::unescape(replacement);
 		  	let regex = Regex::new(&pattern).map_err(|err| 
				MacroError::new("replace".into(), range.clone(), MacroErrorKind::user(
					format!("failed to parse regex: {err}")
				))
 		  	)?;
 		  	let res = regex.replace_all(haystack, replacement);
 		  	Ok(res.into_owned())
		}
	}

	/// Converts the input to an integer, with an optional base to convert from.
	/// # Examples
	/// ```ignore
	/// [int/54.2] // 54
	/// [int/-101/2] // 5
	/// ```
	macro BuiltinInt as "int" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (value, ) = get_args!("int", range, arguments; value);
 		  	if let Some(base) = arguments.get(1) {
				let base = convert_to_number!("int", range; at 2 => base) as u32;
				if !(2 ..= 36).contains(&base) {
					return Err(MacroError::new("int".into(), range, MacroErrorKind::user(
						format!("invalid base {base} (must be between 2 and 36, inclusive)")
					)));
				}
	 			i64::from_str_radix(value, base)
	 				.map(|v| v.to_string())
	 				.map_err(|_| MacroError::new("int".into(), range, MacroErrorKind::user(
	 					format!("failed to convert {value} to a number with base {base}")
	 				)))
		 	} else {
  	 		  	let value = convert_to_number!("int", range; at 1 => value);
 		  		Ok((value as i64).to_string())
 		  	}
		}
	}

	/// Converts the input to a hexadecimal integer.
	/// # Examples
	/// ```ignore
	/// [hex/16] // 10
	/// ```
	macro BuiltinHex as "hex" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (value, ) = get_args!("hex", range, arguments; value);
			let value = convert_to_number!("hex", range; at 1 => value) as i64;
			Ok(format!("{value:x}"))
		}
	}

	
	/// Converts the input to a binary integer.
	/// # Examples
	/// ```ignore
	/// [bin/5] // 101
	/// ```
	macro BuiltinBin as "bin" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (value, ) = get_args!("bin", range, arguments; value);
			let value = convert_to_number!("bin", range; at 1 => value) as i64;
			Ok(format!("{value:b}"))
		}
	}

	
	/// Converts the input to an octal integer.
	/// # Examples
	/// ```ignore
	/// [oct/59] // 73
	/// ```
	macro BuiltinOct as "oct" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (value, ) = get_args!("bin", range, arguments; value);
			let value = convert_to_number!("bin", range; at 1 => value) as i64;
			Ok(format!("{value:o}"))
		}
	}

	/// Converts a unicode codepoint to a character.
	/// Note that this will error for invalid codepoints!
	/// # Examples
	/// ```ignore
	/// [ord/55296] // error: invalid codepoint
	/// [ord/65] // A
	/// ```
	macro BuiltinOrd as "ord" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (value, ) = get_args!("ord", range, arguments; value);
			let value = convert_to_number!("ord", range; at 1 => value) as u32;
			char::from_u32(value)
				.map(|v| v.to_string())
				.ok_or_else(|| MacroError::new("ord".into(), range, MacroErrorKind::user(
					"invalid codepoint"
				)))
		}
	}

	/// Converts a character into its unicode codepoint.
	/// All extraneous characters are discarded.
	/// # Examples
	/// ```ignore
	/// [chr/] // error: no input
	/// [chr/A] // 65
	/// [chr/Among Us] // 65
	/// ```
	macro BuiltinChr as "chr" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (value, ) = get_args!("chr", range, arguments; value);
 		  	value.chars().next()
 		  		.map(|c| (c as u32).to_string())
 		  		.ok_or_else(|| MacroError::new("chr".into(), range, MacroErrorKind::user(
		  			"no input"
		  		)))
		}
	}

	/// Gets the length of the first input.
	/// # Examples
	/// ```ignore
	/// [len/] // 0
	/// [len/abc] // 3
	/// [len/abc/def] // 3
	/// ```
	macro BuiltinLength as "len" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (value, ) = get_args!("len", range, arguments; value);
 		  	Ok(value.chars().count().to_string())
		}
	}

	/// Splits the first input delimited by the second,
	/// then returns the section at the third argument.
	/// # Example
	/// ```ignore
	/// [split/a,b,c/,/1] // b
	/// ```
	macro BuiltinSplit as "split" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (haystack, delimiter, index) = get_args!("split", range, arguments; a, b, c);
			let index = convert_to_number!("split", range; at 1 => index) as usize;
 		  	haystack.split(&**delimiter).nth(index)
				.map(|v| v.to_string())
 		  		.ok_or_else(|| MacroError::new(
 		  			"split".into(), range, MacroErrorKind::user(
 		  				format!("index {index} is out of bounds")
 		  			)
 		  		))
		}		
	}

	/// Selects one of the arguments based on an index on the first.
	/// If the index is #, returns the number of arguments, minus 1 for the #.
	/// # Examples
	/// ```ignore
	/// [select/1/a/b/c] // a
	/// [select/#/one/two/three] // 3
	/// [select/0/it works, but why would you do this?] // 0
	/// [select/5/a/b] // error: index 5 is out of bounds
	/// [select/-1/nope, this isn't python] // error: index 4294967295 is out of bounds
	/// ```
	macro BuiltinSelect as "select" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (index, ) = get_args!("select", range, arguments; a);
 		  	if index == "#" {
 		  		return Ok((arguments.len() - 1).to_string());
 		  	}
			let index = convert_to_number!("select", range; at 1 => index) as usize;
			arguments.get(index)
				.map(|v| v.to_string())
				.ok_or_else(|| MacroError::new(
 		  			"select".into(), range, MacroErrorKind::user(
 		  				format!("index {index} is out of bounds")
 		  			)
 		  		))		  				
		}		
	}

	/// Returns whether two strings are equal.
	/// # Examples
	/// ```ignore
	/// [equal/one/one] // true
	/// [equal/one/two] // false
	/// [equal/1/1] // true
	/// [equal/1/1.0] // false
	/// ```
	macro BuiltinEqual as "equal" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (lhs, rhs) = get_args!("equal", range, arguments; a, b);
			Ok((&**lhs == &**rhs).to_string()) // &** to convert &Cow<str> to &str
		}
	}

	/// Returns whether a number is equal to another.
	/// # Examples
	/// ```ignore
	/// [#equal/1/1.0] // true
	/// [#equal/0.3/[add/0.1/0.2]] // false
	/// [#equal/nan/nan] // false
	/// ```
	macro BuiltinNumEqual as "#equal" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (lhs, rhs) = get_args!("#equal", range, arguments; a, b);
			let lhs = convert_to_number!("#equal", range; at 1 => lhs);
			let rhs = convert_to_number!("#equal", range; at 2 => rhs);
			Ok((lhs == rhs).to_string())
		}
	}

	/// Returns whether a number is greater than another.
	/// # Examples
	/// ```ignore
	/// [greater/1/1] // false
	/// [greater/0.2/0.1] // true
	/// [greater/nan/nan] // false
	/// ```
	macro BuiltinGreater as "greater" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (lhs, rhs) = get_args!("greater", range, arguments; a, b);
			let lhs = convert_to_number!("greater", range; at 1 => lhs);
			let rhs = convert_to_number!("greater", range; at 2 => rhs);
			Ok((lhs > rhs).to_string())
		}
	}

	/// Returns whether a number is less than another.
	/// # Examples
	/// ```ignore
	/// [less/1/1] // false
	/// [less/0.1/0.2] // true
	/// [less/nan/nan] // false
	/// ```
	macro BuiltinLess as "less" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (lhs, rhs) = get_args!("less", range, arguments; a, b);
			let lhs = convert_to_number!("less", range; at 1 => lhs);
			let rhs = convert_to_number!("less", range; at 2 => rhs);
			Ok((lhs < rhs).to_string())
		}
	}

	/// Negates many boolean inputs.
	/// # Example
	/// ```ignore
	/// [not/true] false
	/// [not/true/false] false/true
	/// ```
	macro BuiltinNot as "not" {
		fn apply(&self, _range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
			Ok(
				arguments.iter()
					.map(truthy)
					.map(|v| !v)
					.map(|v| v.to_string())
					.join("/")
			)
		}
	}

	/// Takes the logical AND of an arbitrary number of boolean inputs.
	/// # Example
	/// ```ignore
	/// [and/true/true] true
	/// [and/false/true/true] false
	/// ```
	macro BuiltinAnd as "and" {
		fn apply(&self, _range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
			Ok(
				arguments.iter()
					.map(truthy)
					.reduce(|a, b| a && b)
					.unwrap_or(false)
					.to_string()
			)
		}
	}

	/// Takes the logical OR of an arbitrary number of boolean inputs.
	/// # Example
	/// ```ignore
	/// [or/false/true] true
	/// [or/false/true/true] true
	/// ```
	macro BuiltinOr as "or" {
		fn apply(&self, _range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
			Ok(
				arguments.iter()
					.map(truthy)
					.reduce(|a, b| a || b)
					.unwrap_or(false)
					.to_string()
			)
		}
	}


	/// Takes the logical XOR of an arbitrary number of boolean inputs.
	/// # Example
	/// ```ignore
	/// [xor/false/true] true
	/// [xor/false/true/true] false
	/// ```
	macro BuiltinXor as "xor" {
		fn apply(&self, _range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
			Ok(
				arguments.iter()
					.map(truthy)
					.reduce(|a, b| a ^ b)
					.unwrap_or(false)
					.to_string()
			)
		}
	}

	/// Immediately raises an error.
	/// # Example
	/// ```ignore
	/// [error/oh no!] // error: oh no!
	/// ```
	macro BuiltinError as "error" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
			Err(MacroError::new("error".into(), range, MacroErrorKind::user(
				arguments.first().map_or(String::from("no reason given"), |v| v.to_string())
			)))
		}		
	}

	/// Raises an error if the first argument is not truthy.
	/// # Examples
	/// ```ignore
	/// [assert/true/all good] // <no output>
	/// [assert/false/yikes] // error: yikes
	/// ```
	macro BuiltinAssert as "assert" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (condition, ) = get_args!("assert", range, arguments; a);
			if truthy(condition) {
				Ok(String::new())
			} else {
				Err(MacroError::new("assert".into(), range, MacroErrorKind::user(
					arguments.get(1).map_or(String::from("no reason given"), |v| v.to_string())
				)))
			}
		}
	}

	/// Slices a string.
	/// The first argument is the start, the next is the end, and optionally, the last is the step size.
	/// This works similarly to Python's string slicing rules (and is in fact carried over from it).
	/// # Examples
	/// ```
	/// [slice/abcdefg/1/3] // bcd
	/// [slice/abcde/1/] // bcde
	/// [slice/1,2,3,4/1//2] // 1234
	/// [slice/kcab///-1] // back
	/// ```
	macro BuiltinSlice as "slice" {
		fn apply(&self, range: Range<usize>, arguments: Vec<Cow<'_, str>>) -> Result<String, MacroError> {
 		  	let (haystack, start, end) = get_args!("slice", range, arguments; a, b, c);
			let start = (!start.is_empty())
				.then(|| Ok(convert_to_number!("slice", range; at 2 => start) as usize))
				.transpose()?;
			let end = (!end.is_empty())
				.then(|| Ok(convert_to_number!("slice", range; at 3 => end) as usize))
				.transpose()?;
			let step = arguments.get(3)
					.map(|v| Ok(convert_to_number!("slice", range; at 4 => v) as isize))
					.transpose()?
					.unwrap_or(1);
			if step == 0 {
				return Err(MacroError::new("slice".into(), range, MacroErrorKind::user(
					"cannot have a step length of 0"
				)))
			}
			let Some(slice) = (match (start, end) {
				(None, None) => Some(&haystack[..]),
				(Some(s), None) => haystack.get(s..),
				(None, Some(e)) => haystack.get(..e),
				(Some(s), Some(e)) => haystack.get(s..e)
			}) else {
				return Err(MacroError::new("slice".into(), range, MacroErrorKind::user(
					format!(
						"part of range \"{}..{}\" is out of bounds for string of length {}",
						start.map(|v| v.to_string()).unwrap_or_default(),
						end.map(|v| v.to_string()).unwrap_or_default(),
						haystack.chars().count()
					)
				)))
			};
			if step == 1 {
				// Fast path
				Ok(slice.to_string())
			} else if step == -1 {
				// Slightly slower path
				Ok(slice.chars().rev().collect())
			} else {
				// Slow path
				Ok(
					if step < 0 {
						slice.chars().rev().step_by((-step) as usize).collect()
					} else {
						slice.chars().step_by(step as usize).collect()
					}
				)
			}
 		}
 	}
}