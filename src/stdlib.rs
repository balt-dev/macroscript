use crate::execution::{Macro, MacroError, MacroErrorKind};
use itertools::Itertools;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::str::FromStr;

macro_rules! count {
	($tt: tt $($tts: tt)*) => {
		1 + count!($($tts)*)
	};
	() => {0}
}

macro_rules! require_arguments {
    ($range: ident; $arglist: ident; $name: literal => [$($id: ident),+]) => {{
        const COUNT: usize = count!($($id)+);
        let len = $arglist.len();
        if len < COUNT {
            return Err(MacroError {
                name: $name.into(),
                error_type: MacroErrorKind::NotEnoughArguments {
                	expected: COUNT,
                	found: len
                },
                range: $range,
            });
        }
        let new = $arglist.split_off(COUNT);
		let Ok([$($id),+]): Result<[Cow<'_, str>; COUNT], _> = $arglist.try_into() else {unreachable!()}; 
        $arglist = new;
        ($($id,)+)
    }};
}

struct BuiltinAdd;

impl Macro for BuiltinAdd {
    fn apply(
        &self,
        range: Range<usize>,
        arguments: Vec<Cow<'_, str>>,
    ) -> Result<String, MacroError> {
        let args: f64 = arguments
            .into_iter()
            .enumerate()
            .map(|(idx, arg)| {
                f64::from_str(&arg).map_err(|_| {
                    MacroError {
                    	name: "add".into(),
                        error_type: MacroErrorKind::User {
                        	message: format!("could not convert argument {idx} \"{arg}\" to number") 
                        },
                        range: range.clone(),
                    }
                })
            })
           
            .process_results(|iter| iter.sum())?;

        Ok(args.to_string())
    }
}

struct BuiltinMultiply;

impl Macro for BuiltinMultiply {
    fn apply(
        &self,
        range: Range<usize>,
        arguments: Vec<Cow<'_, str>>,
    ) -> Result<String, MacroError> {
        let args: f64 = arguments
            .into_iter()
            .enumerate()
            .map(|(idx, arg)| {
                f64::from_str(&arg).map_err(|_| {
                    MacroError {
                    	name: "multiply".into(),
                        error_type: MacroErrorKind::User {
                        	message: format!("could not convert argument {idx} \"{arg}\" to number") 
                        },
                        range: range.clone(),
                    }
                })
            })
            .process_results(|iter| iter.product())?;

        Ok(args.to_string())
    }
}

struct BuiltinUnescape;

impl Macro for BuiltinUnescape {
    fn apply(
        &self,
        range: Range<usize>,
        mut arguments: Vec<Cow<'_, str>>,
    ) -> Result<String, MacroError> {
    	let (first_arg, ) =
    		require_arguments!{ range; arguments; "unescape" => [a] };
        let unescaped = crate::parsing::unescape(&first_arg);
        Ok(unescaped.into_owned())
    }
}

/// Adds the standard library's builtin macros to a map of macro names.
pub fn add_stdlib(macros: &mut HashMap<String, &dyn Macro>) {
    macros.insert("add".into(), &BuiltinAdd);
    macros.insert("multiply".into(), &BuiltinMultiply);
    macros.insert("unescape".into(), &BuiltinUnescape);
}
