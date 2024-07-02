use crate::parsing;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// An error that can arise from a macro.
pub struct MacroError {
	/// The name of the macro that failed.
	pub name: String,
    /// The type of error that occurred.
    pub error_type: MacroErrorKind,
    /// The range of the macro that errored.
    pub range: Range<usize>,
}

impl MacroError {
	/// Creates an error.
	#[must_use]
	pub fn new(name: String, range: Range<usize>, kind: MacroErrorKind) -> Self {
		MacroError { name, range, error_type: kind }
	}
}

impl MacroErrorKind {
	/// Creates a user error.
	#[must_use]
	pub fn user(message: impl Into<String>) -> Self {
		MacroErrorKind::User { message: message.into() }
	}

	/// Creates an error about not having enough arguments.
	#[must_use]
	pub fn not_enough_args(expected: usize, found: usize) -> Self {
		MacroErrorKind::NotEnoughArguments { expected, found }
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
/// A kind of error that can occur when a macro is executed.
pub enum MacroErrorKind {
	/// Not enough arguments were supplied.
	NotEnoughArguments { expected: usize, found: usize },
	/// A macro didn't exist.
	Nonexistent,
	/// An error was thrown in the macro.
	User { message: String }
}

impl std::fmt::Display for MacroErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		use MacroErrorKind::*;
		match self {
			NotEnoughArguments { expected, found } =>
				write!(f, "expected {expected} arguments, found {found}"),
			Nonexistent =>
				write!(f, "not found"),
			User { message } =>
				write!(f, "{message}") 
		}
	}	
}

impl std::error::Error for MacroError {}

impl std::fmt::Display for MacroError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error in macro {} at range {:?}: {}", self.name, self.range, self.error_type)
    }
}

/// A trait dictating an object as usable as a macro.
pub trait Macro {
    /// The function where this macro is applied.
    ///
    /// # Errors
    /// If the macro fails to apply, an error will be raised with a message.
    fn apply(
        &self,
        range: Range<usize>,
        arguments: Vec<Cow<'_, str>>,
    ) -> Result<String, MacroError>;
}

macro_rules! throw_error {
	($label: tt, $try_stack: ident, $expr: expr) => {
		let err = $expr;
		if let Some((parent, par_range)) = $try_stack.last_mut() {
			parent.replace_range(par_range.clone(), &format!("false/{}",
				format!("{err}").replace("\\", r"\\").replace("[", r"\[").replace("]", r"\]")
			));
			continue $label;
		}
		return Err($expr);
	};
	((dne) $label: tt, $try_stack: ident, $name: expr; $range: expr) => {
		throw_error!($label, $try_stack, MacroError {
        	name: $name.into(), 
        	error_type: MacroErrorKind::Nonexistent,
       		range: $range
       	})
	};
	((not_enough) $label: tt, $try_stack: ident, $name: literal, $expected: literal, $found: literal, $range: expr) => {
		throw_error!($label, $try_stack, MacroError {
        	name: $name.into(), 
        	error_type: MacroErrorKind::NotEnoughArguments {
        		expected: $expected,
        		found: $found
       		},
       		range: $range
       	})
	};
	((user) $label: tt, $try_stack: ident, $name: literal, $message: literal, $range: expr; $($tt: tt)*) => {
		throw_error!($label, $try_stack, MacroError {
        	name: $name.into(), 
        	error_type: MacroErrorKind::User {
        		message: format!($message, $($tt)*)
       		},
       		range: $range
       	})
	}
}

/// Applies all found macros in the string until none are left.
///
/// # Errors
/// Errors if any macro in the input errors.
pub fn apply_macros(
    input: String,
    macros: &HashMap<String, Box<dyn Macro>, impl std::hash::BuildHasher>,
) -> Result<String, MacroError> {
    let input_len = input.len();
    let mut variables: HashMap<String, String> = HashMap::new();
    let mut try_stack = vec![(input, 0..input_len)];
    'try_loop: while let Some((mut input, range)) = try_stack.pop() {
        while let Some(macro_range) = parsing::find_pair(&input) {
            match &*macro_range.name {
                "try" => {
                    let mac_range = macro_range.range;
                    let Some(new_input) = macro_range.arguments.first() else {
                        throw_error!((not_enough) 
                        	'try_loop, try_stack, "try", 
							1, 0,
                       		mac_range.clone()
                       	);
                    };
                    let new_input = parsing::unescape(new_input).into_owned();
                    try_stack.push((input, range));
                    try_stack.push((new_input, mac_range));
                    continue 'try_loop;
                }
                "load" => {
                    let Some(name) = macro_range.arguments.first() else {
                        throw_error!((not_enough) 
                        	'try_loop, try_stack, "load", 
							1, 0,
                      		macro_range.range.clone()
                       	);
                    };
                    let range = macro_range.range;
                    let Some(value) = variables.get(name.as_ref()) else {
                        throw_error!((user)
                        	'try_loop, try_stack, "load", 
                        	"variable {} does not currently exist",
                       		range.clone();
                       		name
                       	);
                    };
                    input.replace_range(range, value);
                }
                "drop" => {
                    let Some(name) = macro_range.arguments.first() else {
                        throw_error!((not_enough) 
                        	'try_loop, try_stack, "drop", 
							1, 0,
                       		macro_range.range.clone()
                      	);
                    };
                    let range = macro_range.range;
                    variables.remove(name.as_ref());
                    input.replace_range(range, "");
                }
                "store" => {
                    let Some(name) = macro_range.arguments.first() else {
						throw_error!((not_enough) 
                        	'try_loop, try_stack, "store", 
							2, 0,
                       		macro_range.range.clone()
                      	);
                    };
                    let Some(value) = macro_range.arguments.get(1) else {
						throw_error!((not_enough) 
                        	'try_loop, try_stack, "store", 
							2, 1,
                       		macro_range.range.clone()
                      	);
                    };
                    let range = macro_range.range;
                    variables.insert(name.clone().into_owned(), value.clone().into_owned());
                    input.replace_range(range, "");
                }
                "get" => {
                    let Some(name) = macro_range.arguments.first() else {
						throw_error!((not_enough) 
                        	'try_loop, try_stack, "get", 
                        	2, 0,
                        	macro_range.range.clone()
                      	);
                    };
                    let Some(value) = macro_range.arguments.get(1) else {
						throw_error!((not_enough) 
                        	'try_loop, try_stack, "get", 
							2, 1,
                       		macro_range.range.clone()
                      	);
                    };
                    let range = macro_range.range;
                    let result = variables
                        .entry(name.clone().into_owned())
                        .or_insert(value.clone().into_owned());
                    input.replace_range(range, result);
                }
                "is_stored" => {
                    let Some(name) = macro_range.arguments.first() else {
						throw_error!((not_enough) 
                        	'try_loop, try_stack, "is_stored", 
							1, 0,
                       		macro_range.range.clone()
                      	);
                   	};
                    let range = macro_range.range;
                    let exists = variables.contains_key(name.as_ref());
                    input.replace_range(range, &exists.to_string());
                }
                other => {
                    let range = macro_range.range;
                    let Some(mac) = macros.get(other) else {
						throw_error!((dne) 'try_loop, try_stack, other; range.clone());
                    };
                    let replace = mac.apply(range.clone(), macro_range.arguments)?;
                    input.replace_range(range, &replace);
                }
            }
        }
        if let Some((parent, par_range)) = try_stack.last_mut() {
            parent.replace_range(par_range.clone(), &format!("true/{input}"));
        } else {
            return Ok(input);
        }
    }
    unreachable!()
}
