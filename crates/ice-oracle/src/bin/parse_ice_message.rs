use std::io::Read;

use ice_oracle::parser::extract_ice_messages;

/// Standalone ICE message parser: reads stderr from stdin and prints extracted ICE info.
fn main() {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .expect("failed to read stdin");

    let ices = extract_ice_messages(&input);

    if ices.is_empty() {
        println!("No ICE messages found.");
    } else {
        for ice in &ices {
            println!("Location: {}", ice.location);
            println!("Reason:   {}", ice.reason);
            println!();
        }
    }
}
