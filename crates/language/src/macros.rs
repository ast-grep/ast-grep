// Parse the named schema in one place. Emitters only match the stable identity,
// lookup, and declaration facets, so adding a field does not change dispatch.
macro_rules! normalize_languages {
  ($callback:ident, $arguments:tt; $(
    $lang:ident {
      feature: $feature:literal,
      $kind:ident: $implementation:path,
      $(module: $module:ident,)?
      aliases: $aliases:expr,
      extensions: $extensions:expr,
      $(expando: $expando:literal,)?
    }
  ),* $(,)?) => {
    $callback! {
      $arguments;
      $(
        $lang $feature ($aliases, $extensions) {
          $(#[cfg(feature = $feature)] mod $module;)?
          define_language!($lang, $feature, $kind, $implementation $(, $expando)?);
          define_alias!($lang, $feature, $aliases);
        }
      )*
    }
  };
  ($($invalid:tt)*) => {
    compile_error!("expected language records with feature, parser or implementation, optional module, aliases, extensions, and optional expando");
  };
}

macro_rules! define_language {
  ($lang:ident, $feature:literal, parser, $parser:path $(, $expando:literal)?) => {
    #[cfg(feature = $feature)]
    #[derive(Clone, Copy, Debug)]
    pub struct $lang;

    #[cfg(feature = $feature)]
    impl Language for $lang {
      fn kind_to_id(&self, kind: &str) -> u16 {
        self.get_ts_language().id_for_node_kind(kind, true)
      }

      fn field_to_id(&self, field: &str) -> Option<u16> {
        self.get_ts_language().field_id_for_name(field).map(|f| f.get())
      }

      $(
        fn expando_char(&self) -> char { $expando }

        fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
          common::pre_process_pattern(self.expando_char(), query)
        }
      )?

      fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, *self))
      }
    }

    #[cfg(feature = $feature)]
    impl LanguageExt for $lang {
      fn get_ts_language(&self) -> TSLanguage { ($parser).into() }
    }
  };
  ($lang:ident, $feature:literal, implementation, $implementation:path) => {
    #[cfg(feature = $feature)]
    pub use $implementation;
  };
  ($lang:ident, $feature:literal, implementation, $implementation:path, $expando:literal) => {
    compile_error!(concat!(stringify!($lang), ": handwritten implementations own their preprocessing"));
  };
  ($lang:ident, $feature:literal, $kind:ident, $implementation:path $(, $expando:literal)?) => {
    compile_error!(concat!(stringify!($lang), ": expected parser or implementation"));
  };
}

macro_rules! define_alias {
  ($lang:ident, $feature:literal, $aliases:expr) => {
    #[cfg(feature = $feature)]
    impl Alias for $lang {
      const ALIAS: &'static [&'static str] = &$aliases;
    }

    #[cfg(feature = $feature)]
    impl fmt::Display for $lang {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
      }
    }

    #[cfg(feature = $feature)]
    impl<'de> Deserialize<'de> for $lang {
      fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(common::AliasVisitor {
          aliases: Self::ALIAS,
        })?;
        Ok($lang)
      }
    }

    #[cfg(feature = $feature)]
    impl From<$lang> for SupportLang {
      fn from(_: $lang) -> Self {
        Self::$lang
      }
    }
  };
}

macro_rules! emit_registry {
  ((); $(
    $lang:ident $feature:literal ($aliases:expr, $extensions:expr) { $($items:tt)* }
  )*) => {
    #[cfg(any($(feature = $feature),*))]
    mod common;

    $($($items)*)*

    /// Represents all built-in languages enabled in this build.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Hash)]
    pub enum SupportLang {
      $(#[cfg(feature = $feature)] $lang,)*
    }

    impl SupportLang {
      pub const fn all_langs() -> &'static [Self] {
        &[$(#[cfg(feature = $feature)] Self::$lang,)*]
      }
    }

    const fn alias(lang: SupportLang) -> &'static [&'static str] {
      match lang {
        $(#[cfg(feature = $feature)] SupportLang::$lang => $lang::ALIAS,)*
      }
    }

    fn extensions(lang: SupportLang) -> &'static [&'static str] {
      match lang {
        $(#[cfg(feature = $feature)] SupportLang::$lang => &$extensions,)*
      }
    }
  };
}

// Emit the whole match expression; macros cannot expand into individual arms.
macro_rules! emit_dispatch {
  (($me:expr, $method:ident, $arguments:tt); $(
    $lang:ident $feature:literal $lookup:tt $items:tt
  )*) => {
    match *$me {
      $(#[cfg(feature = $feature)] SupportLang::$lang => $lang.$method $arguments,)*
    }
  };
}
