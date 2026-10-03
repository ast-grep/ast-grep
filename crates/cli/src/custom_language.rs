use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum CustomLanguagePolicy {
  /// Load native custom language libraries for this invocation.
  Allow,
  /// Skip native custom language libraries without reporting an error.
  #[default]
  Ignore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomLanguageAction {
  Load,
  Ignore,
}

impl From<CustomLanguagePolicy> for CustomLanguageAction {
  fn from(policy: CustomLanguagePolicy) -> Self {
    match policy {
      CustomLanguagePolicy::Allow => Self::Load,
      CustomLanguagePolicy::Ignore => Self::Ignore,
    }
  }
}

pub fn parse_policy(args: &[String]) -> CustomLanguagePolicy {
  let mut policy = None;
  let mut i = 1;
  while i < args.len() {
    let arg = args[i].as_str();
    if arg == "--" {
      break;
    }
    let value = if arg == "--custom-languages" {
      i += 1;
      args.get(i).map(String::as_str)
    } else {
      arg.strip_prefix("--custom-languages=")
    };
    if let Some(value) = value {
      if policy.is_some() {
        return CustomLanguagePolicy::Ignore;
      }
      policy = match value {
        "allow" => Some(CustomLanguagePolicy::Allow),
        "ignore" => Some(CustomLanguagePolicy::Ignore),
        _ => return CustomLanguagePolicy::Ignore,
      };
    }
    i += 1;
  }
  policy.unwrap_or_default()
}

#[cfg(test)]
mod test {
  use super::*;

  fn parse(args: &[&str]) -> CustomLanguagePolicy {
    parse_policy(
      &args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>(),
    )
  }

  #[test]
  fn test_parse_policy() {
    assert_eq!(parse(&["sg", "scan"]), CustomLanguagePolicy::Ignore);
    assert_eq!(
      parse(&["sg", "scan", "--custom-languages=allow"]),
      CustomLanguagePolicy::Allow,
    );
    assert_eq!(
      parse(&["sg", "run", "--", "--custom-languages", "allow"]),
      CustomLanguagePolicy::Ignore,
    );
  }
}
