use macroscript::execution::apply_macros;
use macroscript::stdlib::add_stdlib;
use std::collections::HashMap;
use std::error::Error;
use std::hash::BuildHasherDefault;

#[test]
pub fn main_test() -> Result<(), Box<dyn Error>> {
	let mut macros = HashMap::with_hasher(BuildHasherDefault::<seahash::SeaHasher>::default());
	add_stdlib(&mut macros);
	assert_eq!(Ok("13".to_string()), apply_macros(r"[unescape/\[add\/5\/5\/3\]]".to_string(), &macros));
	assert_eq!(Ok("13".to_string()), apply_macros(r"[unescape/\[add\/5\/5\/3\]/2]".to_string(), &macros));
	assert!(apply_macros(r"[unescape]".to_string(), &macros).is_err());
	assert_eq!("13463560454117874234", apply_macros(r"[hash/rain world]".to_string(), &macros)?);
	assert_eq!("vaapooreeoon", apply_macros(r"[replace/vaporeon/(\[aeiou\])/$1$1]".to_string(), &macros)?);
	assert_eq!("0.5 inf -inf NaN", apply_macros(r"[divide/1/2] [divide/1/0] [divide/-1/0] [divide/0/0]".to_string(), &macros)?);
	assert_eq!("3735928559 0", apply_macros(r"[int/DEADBEEF/16] [int/nan]".to_string(), &macros)?);
	Ok(())
}
