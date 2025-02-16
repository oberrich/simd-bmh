use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};

#[proc_macro]
pub fn parse_pattern(input: TokenStream) -> TokenStream {
    let pattern = parse_macro_input!(input as LitStr).value().replace(" ", "");

    if pattern.is_empty() {
        panic!("Pattern cannot be empty.");
    }

    let mut bytes = Vec::new();
    let mut masks = Vec::new();
    let mut shift_table = vec![pattern.len(); 256]; // Initialize shift table with maximum skip value

    let mut current_pos = 0;
    let mut best_skip_value = 0u8;
    let mut best_skip_mask = 0xFFu8;
    let mut max_skip = 1usize;
    let mut best_skip_offset = 0usize;

    // Iterate over the pattern to generate byte values, masks, and the skip table
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        let next_ch = chars.peek().cloned();

        match (ch, next_ch) {
            // Case for `??` wildcard (matches any byte)
            ('?', Some('?')) => {
                bytes.push(0x00); // Wildcard byte
                masks.push(0x00); // Full wildcard mask
                chars.next(); // Consume the second `?`
            }
            // Case for `?A` (matches lower nibble, stores upper as wildcard)
            ('?', Some(c)) if c.is_ascii_hexdigit() => {
                let byte = u8::from_str_radix(&c.to_string(), 16).expect("Invalid nibble");
                bytes.push(byte);
                masks.push(0x0F); // Lower nibble match mask
                chars.next(); // Consume the hex character
            }
            // Case for `A?` (matches upper nibble, stores lower as wildcard)
            (c, Some('?')) if c.is_ascii_hexdigit() => {
                let byte = u8::from_str_radix(&c.to_string(), 16).expect("Invalid nibble");
                bytes.push(byte << 4); // Shift the byte to the upper nibble
                masks.push(0xF0); // Upper nibble match mask
                chars.next(); // Consume the `?`
            }
            // Case for exact two-byte hex match, e.g., `AA`, `BB`, etc.
            (c1, Some(c2)) if c1.is_ascii_hexdigit() && c2.is_ascii_hexdigit() => {
                let byte_str = format!("{}{}", c1, c2);
                let byte = u8::from_str_radix(&byte_str, 16).expect("Invalid hex byte");
                bytes.push(byte);
                masks.push(0xFF); // Exact byte match mask
                chars.next(); // Consume the second hex digit
            }
            _ => {
                panic!("Invalid pattern token: {}", ch);
            }
        }

        // Update the skip table for Boyer-Moore-Horspool
        if masks.last() != Some(&0x00) {
            if let Some(last_byte) = bytes.last() {
                let skip_value = current_pos + 1;

                // Check if the current mask is a full byte match (0xFF)
                if *masks.last().unwrap() == 0xFF {
                    if skip_value > max_skip {
                        max_skip = skip_value;
                        best_skip_offset = current_pos; // Track the position of the best skip byte
                        best_skip_value = *last_byte;  // Use the full byte value
                        best_skip_mask = *masks.last().unwrap(); // Use the full byte mask
                    }
                } else if *masks.last().unwrap() == 0xF0 || *masks.last().unwrap() == 0x0F {
                    // If no full byte match, fallback to nibble match (0xF0 or 0x0F)
                    if !best_skip_value == 0 && skip_value > max_skip {
                        max_skip = skip_value;
                        best_skip_offset = current_pos; // Track the position of the best skip byte
                        best_skip_value = *last_byte; // Use the nibble value
                        best_skip_mask = *masks.last().unwrap(); // Use the nibble mask
                    }
                }
            }
        }

        current_pos += 1;
    }

    // Build the shift table: this is the BMH skip table
    for i in 0..bytes.len() - 1 {
        let byte = bytes[i] & masks[i];
        shift_table[byte as usize] = bytes.len() - 1 - i;
    }

    // Return the parsed pattern data as a TokenStream for code generation
    let expanded = quote! {
        Pattern {
            bytes: [#(#bytes),*], // Pattern bytes
            masks: [#(#masks),*], // Masks for matching
            best_skip_value: #best_skip_value, // Best skip byte
            best_skip_mask: #best_skip_mask, // Best skip mask
            max_skip: #max_skip, // Maximum skip distance
            best_skip_offset: #best_skip_offset, // Best skip byte position
            shift_table: [#(#shift_table),*], // Boyer-Moore-Horspool skip table
        }
    };

    TokenStream::from(expanded)
}
