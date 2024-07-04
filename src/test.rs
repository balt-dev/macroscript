//#![cfg(doctest)]
// For some reason, the above doesn't work.
// Instead, this is a hidden export.
#![doc(hidden)]
#![allow(missing_docs)]
use std::{
	collections::HashMap,
	error::Error
};

pub fn test_output(test_string: &str) -> Result<(), Box<dyn Error>> {
	for line in test_string.lines() {
		if line.trim_ascii().is_empty() { continue }
		let Some((start, end)) = line.split_once("->") else {
			panic!("malformed test case: {line}")
		};
		let start = start.trim_ascii();
		let mut end = end.trim_ascii();
		if let Some(idx) = end.find('(') {
			end = end[..idx].trim_ascii();
		}
		let mut macros = HashMap::new();
		crate::add_stdlib(&mut macros);
		let result = crate::apply_macros(start.to_string(), &macros);
		let result = match result {
			Ok(v) if v.is_empty() => "<no output>".to_string(),
			Ok(v) => v.trim_ascii().to_string(),
			Err(e) => format!("error: {}", e.error_type)
		}; 
		assert_eq!(end, result, "test case failed: {end:?} != {result:?}");
	}
	Ok(())
}
