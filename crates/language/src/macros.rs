// Keep the implementation and the registry metadata in one declaration. The
// metadata callback must exist even when the language's feature is disabled.
macro_rules! define_language {
  (
    $lang:ident {
      feature: $feature:literal,
      $(module: $module:ident,)?
      parser: $parser:expr,
      aliases: [$($alias:literal),+ $(,)?],
      extensions: [$($extension:literal),+ $(,)?],
      $(expando: $expando:literal,)?
    }
  ) => {
    define_language!(@metadata $lang, $feature, [$($module)?],
      [$($alias),+], [$($extension),+], $);

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
  (
    $lang:ident {
      feature: $feature:literal,
      module: $module:ident,
      implementation: $implementation:path,
      aliases: [$($alias:literal),+ $(,)?],
      extensions: [$($extension:literal),+ $(,)?],
    }
  ) => {
    define_language!(@metadata $lang, $feature, [$module],
      [$($alias),+], [$($extension),+], $);
    #[cfg(feature = $feature)]
    pub use $implementation;
  };
  // Pass a dollar token explicitly to define the nested callback on stable Rust.
  (@metadata $lang:ident, $feature:literal, [$($module:ident)?],
    [$($alias:literal),+], [$($extension:literal),+], $d:tt
  ) => {
    $(#[cfg(feature = $feature)] mod $module;)?

    #[cfg(feature = $feature)]
    impl Alias for $lang {
      const ALIAS: &'static [&'static str] = &[$($alias),+];
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
        deserializer.deserialize_str(common::AliasVisitor { aliases: Self::ALIAS })?;
        Ok($lang)
      }
    }

    #[cfg(feature = $feature)]
    impl From<$lang> for SupportLang {
      fn from(_: $lang) -> Self { Self::$lang }
    }

    macro_rules! $lang {
      ($d next:ident, $d state:tt) => {
        $d next! {
          $d state
          $lang { feature: $feature, extensions: [$($extension),+] }
        }
      };
    }
  };
}

// Each callback appends one declaration. Only the terminal rule emits Rust
// items, since separate macro invocations cannot append enum variants or arms.
macro_rules! define_registry {
  ($($lang:ident),* $(,)?) => {
    define_registry!(@collect [] [$($lang),*]);
  };
  (@collect [$($entries:tt)*] [$first:ident $(, $rest:ident)*]) => {
    $first!(append_language, ([$($entries)*] [$($rest),*]));
  };
  (@collect [$(
    $lang:ident { feature: $feature:literal, extensions: [$($extension:literal),+] }
  )*] []) => {
    #[cfg(any($(feature = $feature),*))]
    mod common;

    /// Represents all built-in languages enabled in this build.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Hash)]
    pub enum SupportLang {
      $(#[cfg(feature = $feature)] $lang,)*
    }

    impl SupportLang {
      pub const fn all_langs() -> &'static [Self] {
        // The historical enumeration order is alphabetical, whereas enum and
        // Serde indices put Go before Elixir. Sort at compile time to keep both.
        #[cfg(any($(feature = $feature),*))]
        const LANGS: &[SupportLang] = &common::sort_languages(
          [$(#[cfg(feature = $feature)] SupportLang::$lang,)*],
          [$(#[cfg(feature = $feature)] stringify!($lang),)*],
        );
        #[cfg(not(any($(feature = $feature),*)))]
        const LANGS: &[SupportLang] = &[];
        LANGS
      }
    }

    const fn alias(lang: SupportLang) -> &'static [&'static str] {
      match lang {
        $(#[cfg(feature = $feature)] SupportLang::$lang => $lang::ALIAS,)*
      }
    }

    fn extensions(lang: SupportLang) -> &'static [&'static str] {
      match lang {
        $(#[cfg(feature = $feature)] SupportLang::$lang => &[$($extension),+],)*
      }
    }

    define_registry!(@dispatch [$($lang => $feature),*] $);
  };
  (@dispatch [$($lang:ident => $feature:literal),*] $d:tt) => {
    macro_rules! execute_lang_method {
      ($d me:expr, $d method:ident, $d args:tt) => {
        match *$d me {
          $(#[cfg(feature = $feature)]
            SupportLang::$lang => $lang.$d method $d args,)*
        }
      };
    }
  };
}

macro_rules! append_language {
  (([$($entries:tt)*] [$($rest:ident),*]) $($entry:tt)*) => {
    define_registry!(@collect [$($entries)* $($entry)*] [$($rest),*]);
  };
}
