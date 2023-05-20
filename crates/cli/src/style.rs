use std::io::Write;

use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub fn fg_color(color: Color) -> Option<ColorSpec> {
  Some(ColorSpec::new().set_fg(Some(color)).to_owned())
}

pub fn write<S: AsRef<str>>(s: S, color_spec: Option<ColorSpec>) {
  let mut writer = StandardStream::stdout(ColorChoice::Always);

  writer.set_color(&color_spec.unwrap_or_default()).unwrap();
  writer.write_all(s.as_ref().as_bytes()).unwrap();
  writer.reset().unwrap();
}

#[cfg(test)]
mod test {
  //
}
