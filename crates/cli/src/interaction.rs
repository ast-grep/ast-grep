use rprompt::prompt_reply_stdout;
use std::io::Result;

// reference https://stackoverflow.com/a/34837038/2198656
/// clear screen
pub fn clear() {
    print!("{}[2J", 27 as char);
}

pub fn prompt(prompt_text: &str, letters: &str, default: Option<char>) -> Result<char> {
    loop {
        let input = prompt_reply_stdout(prompt_text)?;
        if input.is_empty() && default.is_some() {
            return Ok(default.unwrap());
        }
        if input.len() == 1 && letters.contains(&input) {
            return Ok(input.chars().next().unwrap());
        }
        println!("Come again?")
    }
}
