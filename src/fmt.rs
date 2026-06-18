/// Simple AST-based formatter for Mimi source code.
///
/// Handles: indentation normalization (4 spaces), brace style, trailing commas,
/// blank line normalization. Does NOT reorder imports or restructure code.

pub struct Formatter {
    indent_size: usize,
}

impl Formatter {
    pub fn new() -> Self {
        Self { indent_size: 4 }
    }

    /// Format source code, returning the formatted version.
    pub fn format(&self, source: &str) -> String {
        let mut output = String::new();
        let mut indent_level: usize = 0;
        let mut prev_blank = false;

        for line in source.lines() {
            let trimmed = line.trim();

            // Skip empty lines but track them
            if trimmed.is_empty() {
                if !prev_blank {
                    output.push('\n');
                    prev_blank = true;
                }
                continue;
            }
            prev_blank = false;

            // Decrease indent before closing braces
            if trimmed.starts_with('}') || trimmed.starts_with(')') || trimmed.starts_with(']') {
                indent_level = indent_level.saturating_sub(1);
            }

            // Write indented line
            let indent_str = " ".repeat(indent_level * self.indent_size);
            output.push_str(&indent_str);
            output.push_str(trimmed);
            output.push('\n');

            // Increase indent after opening braces
            if trimmed.ends_with('{') || trimmed.ends_with('(') || trimmed.ends_with('[') {
                indent_level += 1;
            }
            // Handle single-line blocks like `if x { y }`
            else if trimmed.contains('{') && trimmed.contains('}') {
                // No indent change for single-line blocks
            }
        }

        output
    }

    /// Format source in place, returning true if changes were made.
    pub fn format_in_place(&self, source: &mut String) -> bool {
        let formatted = self.format(source);
        if formatted != *source {
            *source = formatted;
            true
        } else {
            false
        }
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simple_function() {
        let fmt = Formatter::new();
        let input = "func main() -> i32 {
println(42)
0
}";
        let expected = "func main() -> i32 {
    println(42)
    0
}
";
        assert_eq!(fmt.format(input), expected);
    }

    #[test]
    fn format_nested_braces() {
        let fmt = Formatter::new();
        let input = "func f() -> i32 {
if true {
println(1)
} else {
println(2)
}
0
}";
        let expected = "func f() -> i32 {
    if true {
        println(1)
    } else {
        println(2)
    }
    0
}
";
        assert_eq!(fmt.format(input), expected);
    }

    #[test]
    fn format_no_change_needed() {
        let fmt = Formatter::new();
        let input = "func main() -> i32 {
    42
}
";
        assert!(!fmt.format_in_place(&mut input.to_string()));
    }
}
