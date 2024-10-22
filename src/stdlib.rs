/*!
Contains the standard library for macroscript.

If you want to see the documentation for all macros at once, see [`DocumentationHelper`].

*/


#![allow(
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use itertools::Itertools;
use std::{
    collections::HashMap,
    str::FromStr,
    hash::{Hasher, BuildHasher}
};
use rand_pcg::Pcg32;
use rand::{Rng, SeedableRng};
use seahash::SeaHasher;
use regex::Regex;

use crate::{execution::{Macro, MacroError, MacroErrorKind}, parsing::unescape, TextMacro};

macro_rules! count {
    ($tt: tt $($tts: tt)*) => {
        1 + count!($($tts)*)
    };
    () => {0}
}

macro_rules! builtin_macros {
    ($($(#[$attr: meta])* macro $id: ident as $name: literal {$inner: item})*) => {$(
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Default, Hash)]
        #[doc = concat!("See the documentation on [`DocumentationHelper`] for documentation on this struct.")]
        pub struct $id;
        
        impl Macro for $id {
            $inner
        }
    )*

        /// Item purely for documentation purposes of the standard library.
        /// Dynamically made for easier browsing.
        /**
# Core macros
Even without the standard library, there are a few core macros that are always included. They are as follows:

## `try`
Executes some escaped macroscript, and returns a boolean value and output.

- If the inner script errors, then the boolean is `false` and the output is the error message.
- If the inner script succeeds, then the boolean is `true` and the output is the result of the inner script.

This is reminiscent of Lua's `pcall` function.

### Examples
```
# use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
[try/\[add\/5\/5\]] -> true/10
[try/\[shl\/5\/100\]] -> false/shift amount of 100 is too large
# "#)}
```

## `load`
Loads a variable's value and returns it. Errors if the variable doesn't exist.

### Examples
```
# use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
[load/x] -> error: variable "x" does not currently exist
[store/x/5][load/x] -> 5
# "#)}
```

## `store`
Stores a value into a variable and returns nothing.

The variable table is global to the `apply_macros` function.

### Example
```
# use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
[store/x/5] -> <no output>
# "#)}
```

## `drop`
Deletes a variable.

### Example
```
# use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
[store/x/5][drop/x][load/x] -> error: variable "x" does not currently exist
# "#)}
```

## `get`
Gets the value of a variable, storing a supplied default and returning it if the variable doesn't exist.

### Example
```
# use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
[get/x/5],[load/x] -> 5,5
# "#)}
```

## `is_stored`
Returns whether a variable currently exists.

### Examples
```
# use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
[is_stored/x] -> false
[store/x/5][is_stored/x] -> true
# "#)}
```
         */
        /// ---
        /// # Standard library
        ///
        /// These macros need to be included using [`crate::add_stdlib`].
        ///
        $(
        /// ---
        ///
        #[doc = concat!("# [`", $name, "`](struct@", stringify!($id), ")")]
        $(#[$attr])*
        ///
        )*
        pub enum DocumentationHelper {}

        /// Adds the standard library's builtin macros to a map of macro names.
        pub fn add(macros: &mut HashMap<String, Box<dyn Macro>, impl BuildHasher>) {
            $(
                macros.insert($name.into(), Box::new($id));
            )*
        }
    }
}

macro_rules! get_args {
    ($name: literal, $arguments: ident; $($ids: ident),+) => {{
        let mut args = $arguments.iter();
        let c = count!($($ids)*);
        get_args!{_recur $name args; c $($ids)* | }
    }};
    (_recur $name: literal $args: ident; $amount: ident $id: ident $($ids: ident)* | $($leftover: ident)*) => {
        let Some($id) = $args.next() else {
              return Err(MacroError::new(
                  $name.into(),
                  MacroErrorKind::not_enough_args($amount, count!($($leftover)*))
              ));
          };
        get_args!{ _recur $name $args; $amount $($ids)* | $($leftover)* $id }
    };
    (_recur $name: literal $args: ident; $amount: ident | $($leftover: ident)*) => {
        ($($leftover,)*)
    }
}

macro_rules! convert_to_number {
    ($name: literal; at $idx: expr => $arg: expr) => {
        convert_to_number!($name; <f64> at $idx => $arg)
    };
    ($name: literal; <$ty: ty> at $idx: expr => $arg: expr) => {{
        let arg = $arg;
        <$ty>::from_str(arg).map_err(|_| {
            MacroError::new(
                $name.into(),
                MacroErrorKind::user(
                    format!("could not convert argument {} \"{arg}\" to {}", $idx, stringify!($ty))
                   )
            )
        })?
    }}
}

fn truthy(string: impl AsRef<str>) -> bool {
    match string.as_ref() {
        "true" | "True" => true,
        v if f64::from_str(v).is_ok_and(|v| v > 0. && !v.is_nan()) => true,
        _ => false
    }
}

builtin_macros! {
    /// Comment. Returns nothing.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [/comment!] -> <no output>
    /// # "#)}
    /// ```
    macro Comment as "" {
        fn apply(&self, _arguments: Vec<&str>) -> Result<String, MacroError> {
            Ok(String::new())
        }        
    }

    /// Reverses the given inputs.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [reverse/one/tw\/o/thr\\ee] -> thr\\ee/tw\/o/one
    /// # "#)}
    /// ```
    macro Reverse as "reverse" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            Ok(
                arguments
                .into_iter()
                .rev()
                .join("/")
            )
        }
    }

    /// Addition. Takes 0 or more numeric arguments and returns their sum.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"    
    /// [add/3/2/3/5/3] -> 16
    /// [add/5] -> 5
    /// [add] -> 0
    /// [add/a/b] -> error: could not convert argument 1 "a" to f64
    /// # "#)}
    /// ```
    macro Add as "add" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments
                .iter()
                .enumerate()
                .map(|(idx, arg)| {
                    Ok(convert_to_number!("add"; at idx+1 => arg))
                })
                .process_results(|iter| iter.sum())
                .map(|sum: f64| sum.to_string())
        }
    }

    /// Multiplicaton. Takes 0 or more numeric arguments and returns their product.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [multiply/1/2/3/4/5] -> 120
    /// [multiply/5] -> 5
    /// [multiply] -> 1
    /// # "#)}
    /// ```
    macro Multiply as "multiply" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments
                .iter()
                .enumerate()
                .map(|(idx, arg)| {
                    Ok(convert_to_number!("multiply"; at idx+1 => arg))
                })
                .process_results(|iter| iter.product())
                .map(|product: f64| product.to_string())
        }
    }

    /// Unescapes its input.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [unescape/among\/us] -> among/us
    /// [unescape/[if/true/\[add\/1\/1\]/\[add\/2\/1\]]] -> 2
    /// # "#)}
    /// ```
    macro Unescape as "unescape" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (first_arg, ) = get_args!("unescape", arguments; first_arg);
            Ok(unescape(first_arg).to_string())
        }
    }

    /// Basic alternation. Chooses between all even arguments with the condition of the odd ones,
    ///  with the last as a base case.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [if/true/a/true/b/c] -> a
    /// [if/false/a/true/b/c] -> b
    /// [if/false/a/false/b/c] -> c
    /// [if/false/a/false/b] -> error: all conditions exhausted
    /// [if/c] -> c
    /// # "#)}
    /// ```
    macro If as "if" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            let mut chunks = arguments.chunks_exact(2);
            // Technically refutable pattern
            while let Some([condition, value]) = chunks.next() {
                if truthy(condition) {
                    return Ok((*value).to_string());
                }
            }
            if let [end] = chunks.remainder() {
                Ok((*end).to_string())
            } else {
                Err(MacroError {
                    name: "if".into(),
                    error_type: MacroErrorKind::User {
                        message: "all conditions exhausted".into()
                    }
                })
            }
        }
    }

    /// Returns whether a string is "truthy", i.e. whether it converts to true or false.
    /// Truthy strings have to be either "True", "true", or a number greater than 0.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [truthy/1] -> true
    /// [truthy/0] -> false
    /// [truthy/ture] -> false
    /// [truthy/among us] -> false
    /// [truthy/True/true] -> true/true
    /// # "#)}
    /// ```
    macro Truthy as "truthy" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               Ok(arguments.into_iter().map(truthy).join("/"))
          }        
    }

    /// Returns whether a string can be converted to a number.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [is_number/1] -> true
    /// [is_number/abc/2] -> false/true
    /// # "#)}
    /// ```
    macro IsNumber as "is_number" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
                Ok(arguments.into_iter().map(|v| f64::from_str(v).is_ok()).join("/"))
        }
    }

    /// Raises a number to the power of another.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [pow/7/2] -> 49
    /// # "#)}
    /// ``` 
    macro Pow as "pow" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (base, exp) = get_args!("pow", arguments; base, exp);
               let base = convert_to_number!("pow"; at 1 => base);
               let exp = convert_to_number!("pow"; at 2 => exp);
            Ok(base.powf(exp).to_string())
        }
    }

    /// Subtracts a number from another.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [subtract/7/2] -> 5
    /// [subtract/3/5] -> -2
    /// # "#)}
    /// ``` 
    macro Sub as "subtract" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (lhs, rhs) = get_args!("subtract", arguments; a, b);
               let lhs = convert_to_number!("subtract"; at 1 => lhs);
               let rhs = convert_to_number!("subtract"; at 2 => rhs);
            Ok((lhs - rhs).to_string())
        }
    }
    
    /// Divides a number by another.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [divide/5/2] -> 2.5 
    /// [divide/3/5] -> 0.6
    /// [divide/1/0] -> inf
    /// [divide/-1/0] -> -inf
    /// [divide/0/0] -> NaN
    /// # "#)}
    /// ``` 
    macro Div as "divide" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (lhs, rhs) = get_args!("divide", arguments; a, b);
               let lhs = convert_to_number!("divide"; at 1 => lhs);
               let rhs = convert_to_number!("divide"; at 2 => rhs);
            Ok((lhs / rhs).to_string())
        }
    }
    
    /// Takes the modulus of one number with respect to another.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [mod/5/2] -> 1
    /// [mod/-3/5] -> 2
    /// # "#)}
    /// ``` 
    macro Modulus as "mod" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (lhs, rhs) = get_args!("mod", arguments; a, b);
               let lhs = convert_to_number!("mod"; at 1 => lhs);
               let rhs = convert_to_number!("mod"; at 2 => rhs);
            Ok(lhs.rem_euclid(rhs).to_string())
        }
    }

    
    /// Takes the logarithm of a number. The base is optional, and defaults to [`std::f64::consts::E`].
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [log/5] -> 1.6094379124341003
    /// [log/16/2] -> 4
    /// # "#)}
    /// ``` 
    macro Log as "log" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (value, ) = get_args!("log", arguments; value);
               let value = convert_to_number!("log"; at 1 => value);
               let base = if let Some(base) = arguments.get(1) {
                   convert_to_number!("log"; at 2 => base)
               } else {
                   std::f64::consts::E
               };
            Ok(value.log(base).to_string())
        }
    }

    /// Gets a random number on the range [0, 1).
    /// A seed can optionally be supplied.
    /// ### Examples
    /// ```
    /// # /*
    /// [rand] -> ?
    /// # */
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [rand/among us] -> 0.22694492387911513
    /// # "#)}
    /// ``` 
    macro Rand as "rand" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
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

    /// Hashes many values, returning 64-bit integers.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [hash/rain world/brain rot] -> -4983183619591677382/-1860790453662518022
    /// # "#)}
    /// ```
    macro Hash as "hash" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
           Ok(
                arguments.iter().map(|value| {
                    let mut hasher = SeaHasher::new();
                    hasher.write(value.as_bytes());
                    hasher.finish() as i64
               }).join("/")
            )
        }
    }

    /// Replaces all matches of a regular expression with a pattern.
    /// Both the pattern and replacement are unescaped.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [replace/vaporeon/(\[aeiou\])/$1$1] -> vaapooreeoon
    /// [replace/porygon/\[o/e] -> error: unclosed character class
    /// # "#)}
    /// ```
    macro Replace as "replace" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (haystack, pattern, replacement) = get_args!("hash", arguments; a, b, c);
            let pattern = unescape(pattern);
            let replacement = unescape(replacement);
               let regex = Regex::new(&pattern).map_err(|err| {
                let disp = match err {
                    regex::Error::Syntax(err) => {
                        let err_string = err.to_string();
                        let last_line = err_string.lines().last().unwrap();
                        last_line[7..].to_string()
                    },
                    regex::Error::CompiledTooBig(limit) =>
                        format!("compiled regex exceeds size limit of {limit} bytes"),
                    _ => err.to_string()
                };
                MacroError::new("replace".into(), MacroErrorKind::user(disp))
            })?;
               let res = regex.replace_all(haystack, replacement);
               Ok(res.into_owned())
        }
    }

    /// Converts the input to an integer, with an optional base to convert from.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [int/54.2] -> 54
    /// [int/-101/2] -> -5
    /// [int/E621/16] -> 58913
    /// # "#)}
    /// ```
    macro Int as "int" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (value, ) = get_args!("int", arguments; value);
               if let Some(base) = arguments.get(1) {
                let base = convert_to_number!("int"; <u32> at 2 => base);
                if !(2 ..= 36).contains(&base) {
                    return Err(MacroError::new("int".into(), MacroErrorKind::user(
                        format!("invalid base {base} (must be between 2 and 36, inclusive)")
                    )));
                }
                 i64::from_str_radix(value, base)
                     .map(|v| v.to_string())
                     .map_err(|_| MacroError::new("int".into(), MacroErrorKind::user(
                         format!("failed to convert {value} to a number with base {base}")
                     )))
             } else {
                     let value = convert_to_number!("int"; at 1 => value) as i64;
                   Ok(value.to_string())
               }
        }
    }

    /// Converts the input to a hexadecimal integer.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [hex/16] -> 10
    /// [hex/255/5] -> FF/5
    /// # "#)}
    /// ```
    macro Hex as "hex" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(format!("{:X}", convert_to_number!("hex"; <i64> at idx + 1 => value)))
                ).process_results(|mut iter| iter.join("/"))
        }
    }

    
    /// Converts the input to a binary integer.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [bin/5] -> 101
    /// [bin/7/8] -> 111/1000
    /// # "#)}
    /// ```
    macro Bin as "bin" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(format!("{:b}", convert_to_number!("bin"; <i64> at idx + 1 => value)))
                ).process_results(|mut iter| iter.join("/"))
        }
    }

    
    /// Converts the input to an octal integer.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [oct/59] -> 73
    /// [oct/1777/755] -> 3361/1363
    /// # "#)}
    /// ```
    macro Oct as "oct" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(format!("{:o}", convert_to_number!("oct"; <i64> at idx + 1 => value)))
                ).process_results(|mut iter| iter.join("/"))
        }
    }

    /// Converts a unicode codepoint to a character.
    /// Note that this will error for invalid codepoints!
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [chr/55296] -> error: invalid codepoint at argument 1
    /// [chr/65] -> A
    /// [chr/65/109/111/110/103/32/85/115] -> Among Us
    /// # "#)}
    /// ```
    macro Chr as "chr" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments
                .iter().enumerate()
                .map(|(idx, chr)| {
                    let ord = convert_to_number!("chr"; <u32> at idx + 1 => *chr);
                    char::from_u32(ord).ok_or_else(|| MacroError::new("chr".into(), MacroErrorKind::user(
                        format!("invalid codepoint at argument {}", idx + 1)
                    )))
                }).collect()
        }
    }

    /// Converts characters into their unicode codepoints.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [ord/] -> <no output>
    /// [ord/A] -> 65
    /// [ord/Among Us] -> 65/109/111/110/103/32/85/115
    /// # "#)}
    /// ```
    macro Ord as "ord" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (value, ) = get_args!("ord", arguments; value);
               Ok(value.chars()
                   .map(|c| (c as u32).to_string())
                   .join("/"))
        }
    }

    /// Gets the length of the inputs.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [len/] -> 0
    /// [len/abc] -> 3
    /// [len/abc/de] -> 3/2
    /// # "#)}
    /// ```
    macro Length as "len" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               Ok(arguments.into_iter().map(|c| c.chars().count().to_string()).join("/"))
        }
    }

    /// Splits the first input delimited by the second,
    /// then returns the section at the third argument.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [split/a,b,c/,/1] -> b
    /// # "#)}
    /// ```
    macro Split as "split" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (haystack, delimiter, index) = get_args!("split", arguments; a, b, c);
            let index = convert_to_number!("split"; <usize> at 1 => index);
               haystack.split(&**delimiter).nth(index)
                .map(ToString::to_string)
                   .ok_or_else(|| MacroError::new(
                       "split".into(), MacroErrorKind::user(
                           format!("index {index} is out of bounds")
                       )
                   ))
        }        
    }

    /// Selects one of the arguments based on an index on the first.
    /// If the index is `#`, returns the number of arguments, minus 1 for the `#`.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [select/1/a/b/c] -> a
    /// [select/#/one/two/three] -> 3
    /// [select/0/it works, but why would you do this?] -> 0
    /// [select/5/a/b] -> error: index 5 is out of bounds
    /// [select/-1/nope, this isn't python] -> error: could not convert argument 1 "-1" to usize
    /// # "#)}
    /// ```
    macro Select as "select" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (index, ) = get_args!("select", arguments; a);
               if *index == "#" {
                   return Ok((arguments.len() - 1).to_string());
               }
            let index = convert_to_number!("select"; <usize> at 1 => index);
            arguments.get(index)
                .map(ToString::to_string)
                .ok_or_else(|| MacroError::new(
                       "select".into(), MacroErrorKind::user(
                           format!("index {index} is out of bounds")
                       )
                   ))
        }        
    }

    /// Returns whether many strings are equal.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [equal/one/one] -> true
    /// [equal/one/two/three] -> false
    /// [equal/1/1] -> true
    /// [equal/1/1.0] -> false
    /// # "#)}
    /// ```
    macro Equal as "equal" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            Ok(arguments.windows(2).all(|w| *w[0] == *w[1]).to_string()) // ** to convert &Cow<str> to str
        }
    }

    /// Returns whether a number is equal to another.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [#equal/1/1.0] -> true
    /// [#equal/0.3/[add/0.1/0.2]] -> false
    /// [#equal/nan/nan] -> false
    /// # "#)}
    /// ```
    macro NumEqual as "#equal" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (lhs, rhs) = get_args!("#equal", arguments; a, b);
            let lhs = convert_to_number!("#equal"; at 1 => lhs);
            let rhs = convert_to_number!("#equal"; at 2 => rhs);
            Ok((lhs == rhs).to_string())
        }
    }

    /// Returns whether a number is greater than another.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [greater/1/1] -> false
    /// [greater/0.2/0.1] -> true
    /// [greater/nan/nan] -> false
    /// # "#)}
    /// ```
    macro Greater as "greater" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (lhs, rhs) = get_args!("greater", arguments; a, b);
            let lhs = convert_to_number!("greater"; at 1 => lhs);
            let rhs = convert_to_number!("greater"; at 2 => rhs);
            Ok((lhs > rhs).to_string())
        }
    }

    /// Returns whether a number is less than another.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [less/1/1] -> false
    /// [less/0.1/0.2] -> true
    /// [less/nan/nan] -> false
    /// # "#)}
    /// ```
    macro Less as "less" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (lhs, rhs) = get_args!("less", arguments; a, b);
            let lhs = convert_to_number!("less"; at 1 => lhs);
            let rhs = convert_to_number!("less"; at 2 => rhs);
            Ok((lhs < rhs).to_string())
        }
    }

    /// Negates many boolean inputs.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [not/1.0] -> false
    /// [not/true/false/3.0/-5.9] -> false/true/false/true
    /// # "#)}
    /// ```
    macro Not as "not" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
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
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [and/true/true] -> true
    /// [and/false/true/true] -> false
    /// # "#)}
    /// ```
    macro And as "and" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
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
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [or/false/true] -> true
    /// [or/false/true/true] -> true
    /// # "#)}
    /// ```
    macro Or as "or" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
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
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [xor/false/true] -> true
    /// [xor/false/true/true] -> false
    /// # "#)}
    /// ```
    macro Xor as "xor" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            Ok(
                arguments.iter()
                    .map(truthy)
                    .reduce(|a, b| a ^ b)
                    .unwrap_or(false)
                    .to_string()
            )
        }
    }

    /// Takes the bitwise NOT of many 64-bit signed integer inputs.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [#not/0] -> -1 (0b00...0 -> 0b11...1)
    /// [#not/5/-4] -> -6/3
    /// # "#)}
    /// ```
    macro BitNot as "#not" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(!convert_to_number!("abs"; <i64> at idx + 1 => value))
                ).process_results(|mut iter| iter.join("/"))
        }        
    }

    /// Takes the bitwise AND of many 64-bit signed integer inputs.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [#and/11/5] -> 1 (0b1011 & 0b0101)
    /// [#and/8/13/7] -> 0 (0b1000 & 0b1101 & 0b0111)
    /// # "#)}
    /// ```
    macro BitAnd as "#and" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)| Ok(convert_to_number!("#and"; <i64> at idx + 1 => value)))
                .process_results(|iter| iter.reduce(|a, b| a & b).unwrap_or(0).to_string())
        }        
    }
    
    /// Takes the bitwise OR of many 64-bit signed integer inputs.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [#or/5/3] -> 7 (0b0101 | 0b0011)
    /// [#or/9/5/2] -> 15 (0b1001 | 0b0101 | 0b0010)
    /// # "#)}
    /// ```
    macro BitOr as "#or" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)| Ok(convert_to_number!("#or"; <i64> at idx + 1 => value)))
                .process_results(|iter| iter.reduce(|a, b| a | b).unwrap_or(0).to_string())
        }        
    }

    
    /// Takes the bitwise XOR of two 64-bit signed integer inputs.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [#xor/5/3] -> 6 (0b0101 ^ 0b0011)
    /// [#xor/8/11/5] -> 6 (0b1000 ^ 0b1011 ^ 0b0101)
    /// # "#)}
    /// ```
    macro BitXor as "#xor" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)| Ok(convert_to_number!("#xor"; <i64> at idx + 1 => value)))
                .process_results(|iter| iter.reduce(|a, b| a ^ b).unwrap_or(0).to_string())
        }        
    }

    /// Shifts the first argument's bits to the left by the second argument.
    /// The second argument may not be greater than 63.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [shl/5/2] -> 20 (0b101 -> 0b10100)
    /// [shl/-9223372036854775808/1] -> 0 (0b100...0 -> 0b00...0)
    /// # "#)}
    /// ```
    macro ShiftLeft as "shl" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            let (lhs, rhs) = get_args!("shl", arguments; a, b);
            let lhs = convert_to_number!("shl"; <i64> at 1 => lhs) as u64;
            let rhs = convert_to_number!("shl"; <u32> at 2 => rhs);
            lhs.checked_shl(rhs)
                .map(|v| (v as i64).to_string())
                .ok_or_else(|| MacroError::new(
                    "shl".into(),
                    MacroErrorKind::user(format!("shift amount of {rhs} is too large"))
                ))
        }
    }

    
    /// Shifts the first argument's bits to the right by the second argument.
    /// The second argument may not be greater than 63.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [shr/-9223372036854775808/1] -> 4611686018427387904 (0b100...0 -> 0b0100...0)
    /// # "#)}
    /// ```
    macro ShiftRight as "shr" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            let (lhs, rhs) = get_args!("shr", arguments; a, b);
            let lhs = convert_to_number!("shr"; <i64> at 1 => lhs) as u64;
            let rhs = convert_to_number!("shr"; <u32> at 2 => rhs);
            lhs.checked_shr(rhs)
                .map(|v| (v as i64).to_string())
                .ok_or_else(|| MacroError::new(
                    "shr".into(),
                    MacroErrorKind::user(format!("shift amount of {rhs} is too large"))
                ))
        }
    }

    
    /// Shifts the first argument's bits to the right by the second argument, keeping the sign bit.
    /// The second argument may not be greater than 63.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [#shr/-9223372036854775808/1] -> -4611686018427387904 (0b100...0 -> 0b1100...0)
    /// # "#)}
    /// ```
    macro ArithmeticShiftRight as "#shr" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            let (lhs, rhs) = get_args!("#shr", arguments; a, b);
            let lhs = convert_to_number!("#shr"; <i64> at 1 => lhs);
            let rhs = convert_to_number!("#shr"; <u32> at 2 => rhs);
            lhs.checked_shr(rhs)
                .map(|v| v.to_string())
                .ok_or_else(|| MacroError::new(
                    "#shr".into(),
                    MacroErrorKind::user(format!("shift amount of {rhs} is too large"))
                ))
        }
    }

    /// Gets the absolute value of many numbers.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [abs/-5] -> 5
    /// [abs/NaN/-inf] -> NaN/inf
    /// # "#)}
    /// ```
    macro Abs as "abs" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(convert_to_number!("abs"; at idx + 1 => value).abs().to_string())
                ).process_results(|mut iter| iter.join("/"))
        }    
    }
    
    /// Gets the sine of many numbers.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [int/[sin/3.14159]] -> 0
    /// # "#)}
    /// ```
    macro Sine as "sin" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(convert_to_number!("sin"; at idx + 1 => value).sin().to_string())
                ).process_results(|mut iter| iter.join("/"))
        }    
    }

    /// Gets the cosine of many numbers.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [int/[add/-0.01/[cos/3.14159]]] -> -1
    /// # "#)}
    /// ```
    macro Cosine as "cos" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(convert_to_number!("cos"; at idx + 1 => value).cos().to_string())
                ).process_results(|mut iter| iter.join("/"))
        }    
    }
    
    /// Gets the tangent of many numbers.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [int/[multiply/2/[tan/1]]] -> 3
    /// # "#)}
    /// ```
    macro Tangent as "tan" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(convert_to_number!("tan"; at idx + 1 => value).tan().to_string())
                ).process_results(|mut iter| iter.join("/"))
        }
    }

    
    /// Gets the inverse sine of many numbers.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [asin/0/1] -> 0/1.5707963267948966
    /// # "#)}
    /// ```
    macro InvSine as "asin" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(convert_to_number!("asin"; at idx + 1 => value).asin().to_string())
                ).process_results(|mut iter| iter.join("/"))
        }    
    }

    /// Gets the inverse cosine of many numbers.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [acos/1/0] -> 0/1.5707963267948966
    /// # "#)}
    /// ```
    macro InvCosine as "acos" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(convert_to_number!("sin"; at idx + 1 => value).acos().to_string())
                ).process_results(|mut iter| iter.join("/"))
        }    
    }
    
    /// Gets the inverse tangent of a number.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [int/[atan/1.5708]] -> 1
    /// # "#)}
    /// ```
    macro InvTangent as "atan" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            arguments.iter().enumerate()
                .map(|(idx, value)|
                    Ok(convert_to_number!("atan"; at idx + 1 => value).atan().to_string())
                ).process_results(|mut iter| iter.join("/"))
        }
    }

    /// Immediately raises an error. The error message is unescaped.
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [error/oh no!] -> error: oh no!
    /// # "#)}
    /// ```
    macro Error as "error" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            Err(MacroError::new("error".into(), MacroErrorKind::user(
                arguments.first().map_or(String::from("no reason given"), ToString::to_string)
            )))
        }        
    }

    /// Raises an error if the first argument is not truthy.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [assert/1/all good] -> <no output>
    /// [assert/false/yikes] -> error: yikes
    /// # "#)}
    /// ```
    macro Assert as "assert" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (condition, ) = get_args!("assert", arguments; a);
            if truthy(condition) {
                Ok(String::new())
            } else {
                Err(MacroError::new("assert".into(), MacroErrorKind::user(
                    arguments.get(1).map_or(String::from("no reason given"), ToString::to_string)
                )))
            }
        }
    }

    /// Slices a string.
    /// The first argument is the start, the next is the end, and optionally, the last is the step size.
    /// This works similarly to Python's string slicing rules (and is in fact carried over from it).
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [slice/abcdefg/1/4] -> bcd
    /// [slice/abcde/1/] -> bcde
    /// [slice/1,2,30,45///2] -> 123,5
    /// [slice/kcab///-1] -> back
    /// # "#)}
    /// ```
    macro Slice as "slice" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
               let (haystack, start, end) = get_args!("slice", arguments; a, b, c);
            let start = (!start.is_empty())
                .then(|| Ok(convert_to_number!("slice"; <usize> at 2 => start)))
                .transpose()?;
            let end = (!end.is_empty())
                .then(|| Ok(convert_to_number!("slice"; <usize> at 3 => end)))
                .transpose()?;
            let step = arguments.get(3)
                    .map(|v| Ok(convert_to_number!("slice"; <isize> at 4 => v)))
                    .transpose()?
                    .unwrap_or(1);
            if step == 0 {
                return Err(MacroError::new("slice".into(), MacroErrorKind::user(
                    "cannot have a step length of 0"
                )))
            }
            let Some(slice) = (match (start, end) {
                (None, None) => Some(&haystack[..]),
                (Some(s), None) => haystack.char_indices().nth(s).and_then(|(s, _)| haystack.get(s..)),
                (None, Some(e)) => haystack.char_indices().nth(e).and_then(|(e, _)| haystack.get(..e)),
                (Some(s), Some(e)) => haystack.char_indices().nth(s)
                    .and_then(|(s, _)| Some((s, haystack.char_indices().nth(e)?)))
                    .and_then(|(s, (e, _))| haystack.get(s..e))
            }) else {
                return Err(MacroError::new("slice".into(), MacroErrorKind::user(
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

    /// Returns the start location of the second argument in the first.
    /// Returns -1 if it couldn't be found.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [find/homeowner/meow] -> 2
    /// [find/clubstep monster/end] -> -1
    /// # "#)}
    /// ```
     macro Find as "find" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
              let (haystack, needle) = get_args!("find", arguments; a, b);
            
            Ok(haystack.find(&**needle).map_or(-1, |v| {
                haystack[..v].chars().count() as isize
            }).to_string())
        }
    }

    /// Returns the number of disjoint occurrences of the second argument in the first.
    /// Returns 0 if none were found.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [count/Pacific Ocean/c] -> 3
    /// [count/hellololo/lol] -> 1
    /// # "#)}
    /// ```
    macro Count as "count" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
              let (haystack, needle) = get_args!("count", arguments; a, b);
            Ok(haystack.matches(&**needle).count().to_string())
        }
    }

    /// Joins all arguments with the unescaped first argument.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [join/:/red/left/sleep] -> red:left:sleep
    /// [join/\/\//dou/ble] -> dou//ble
    /// # "#)}
    /// ```
    macro Join as "join" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
              let (delimiter, ) = get_args!("join", arguments; a);
              Ok(arguments.iter().skip(1).join(&unescape(delimiter)))
        }        
    }

    /// Escapes the first argument.
    ///
    /// ### Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [escape/add/5/3] -> add\/5\/3
    /// # "#)}
    /// ```
    macro Escape as "escape" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
              let raw = arguments.join("/");
              Ok(raw.replace('\\', r"\\").replace('/', r"\/").replace('[', r"\[").replace(']', r"\]"))
        }
    }

    /// Repeats the first argument N times, where N is the second argument, optionally joined by the third argument.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [repeat/5/5/:] -> 5:5:5:5:5
    /// [store/x/0][unescape/[repeat/\[store\/x\/\[add\/\[load\/x\]\/1\]\]\[load\/x\]/5]] -> 12345
    /// # "#)}
    /// ```
    macro Repeat as "repeat" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
              let (target, count) = get_args!("repeat", arguments; a, b);
            let count = convert_to_number!("repeat"; <usize> at 2 => count);
            Ok(std::iter::repeat(target).take(count).join(arguments.get(2).map_or("", |v| &**v)))
        }
    }

    /// Turns the input into lowercase.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [lower/VVVVVV/GO PLAY IT] -> vvvvvv/go play it
    /// [lower/ὈΔΥΣΣΕΎΣ] -> ὀδυσσεύς
    /// # "#)}
    /// ```
    macro Lower as "lower" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            Ok(arguments.into_iter().map(str::to_lowercase).join("/"))
        }
    }

    /// Turns the input into uppercase.
    /// ### Examples
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [upper/vvvvvv/go play it] -> VVVVVV/GO PLAY IT
    /// [upper/tschüß] -> TSCHÜSS
    /// # "#)}
    /// ```
    macro Upper as "upper" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            Ok(arguments.into_iter().map(str::to_uppercase).join("/"))
        }
    }

    /// Maps an escaped text macro over all of the inputs, returning the results as outputs.
    /// # Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [map/\[multiply\/$1\/2\]/1/2/3] -> 2/4/6
    /// # "#)}
    /// ```
    macro Map as "map" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            if arguments.len() == 1 { return Ok(String::new()) }
            let (mac, ) = get_args!("map", arguments; a);
            let mac = TextMacro::new(unescape(mac));
            arguments
                .iter()
                .skip(1)
                .map(|v| mac.apply(vec![v]))
                .process_results(|mut v| v.join("/"))
        }
    }

    /// Performs a fold with an escaped text macro over all of the inputs, taking the first as a base case.
    /// # Example
    /// ```
    /// # use macroscript::test::test_output; fn main() -> Result<(), Box<dyn std::error::Error>> { test_output(r#"
    /// [fold/\[add\/$1\/$2\]/0/1/2/3] -> 6
    /// # "#)}
    /// ```
    macro Fold as "fold" {
        fn apply(&self, arguments: Vec<&str>) -> Result<String, MacroError> {
            let (mac, base) = get_args!("map", arguments; a, b);
            let mac = TextMacro::new(unescape(mac));
            arguments
                .iter()
                .skip(2)
                .try_fold((*base).to_string(), |a, b| mac.apply(vec![&a, *b]))
        }
    }
}
