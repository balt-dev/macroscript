# macroscript
---
This is a full reimplementation of [Robot Is Chill](https://github.com/balt-dev/robot-is-chill)'s macro-based programming language.

## Changes
A few things have been changed between RIC's implementation and this one.
The most notable ones include:
- No complex numbers
- Arguments are unescaped by default
- No runtime limits
- More builtin macros
- Numbers are limited to f64s
- Text macros (e.g. `double: [multiply/$1/2]`) aren't included by default
  - In order to use text macros, they have to be added using [`TextMacro`].

## Example
```
use macroscript::{apply_macros, add_stdlib};
use std::collections::HashMap;

let mut macros = HashMap::new();
add_stdlib(&mut macros);

let input = "[add/5/3]".to_string();
let result = apply_macros(input, &macros).unwrap();
assert_eq!(result, "8");
```
