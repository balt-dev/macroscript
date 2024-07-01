use macroscript::execution::apply_macros;
use macroscript::stdlib::add_stdlib;
use std::collections::HashMap;
use std::error::Error;

#[test]
pub fn main_test() -> Result<(), Box<dyn Error>> {
	let mut macros = HashMap::new();
	add_stdlib(&mut macros);
	assert_eq!(Ok("13".to_string()), apply_macros(r"[unescape/\[add\/5\/5\/3\]]".to_string(), &macros));
	assert_eq!(Ok("13".to_string()), apply_macros(r"[unescape/\[add\/5\/5\/3\]/2]".to_string(), &macros));
	assert!(apply_macros(r"[unescape]".to_string(), &macros).is_err());

	Ok(())
}
