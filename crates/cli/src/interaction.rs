use rprompt::prompt_reply_stdout;
use std::io::Result;

// https://github.com/console-rs/console/blob/be1c2879536c90ffc2b54938b5964084f5fef67d/src/common_term.rs#L56
/// clear screen
pub fn clear() {
    print!("\r\x1b[2J\r\x1b[H");
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
