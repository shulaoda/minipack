use oxc::transformer::ESTarget as OxcEstarget;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Default)]
pub enum ESTarget {
  Es5,
  Es2015,
  Es2016,
  Es2017,
  Es2018,
  Es2019,
  Es2020,
  Es2021,
  Es2022,
  Es2023,
  Es2024,
  #[default]
  EsNext,
}

impl FromStr for ESTarget {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "es5" => Ok(Self::Es5),
      "es2015" => Ok(Self::Es2015),
      "es2016" => Ok(Self::Es2016),
      "es2017" => Ok(Self::Es2017),
      "es2018" => Ok(Self::Es2018),
      "es2019" => Ok(Self::Es2019),
      "es2020" => Ok(Self::Es2020),
      "es2021" => Ok(Self::Es2021),
      "es2022" => Ok(Self::Es2022),
      "es2023" => Ok(Self::Es2023),
      "es2024" => Ok(Self::Es2024),
      "esnext" => Ok(Self::EsNext),
      _ => Err(format!("Invalid target \"{s}\".")),
    }
  }
}

impl From<ESTarget> for OxcEstarget {
  fn from(value: ESTarget) -> Self {
    match value {
      ESTarget::Es5 => Self::ES5,
      ESTarget::Es2015 => Self::ES2015,
      ESTarget::Es2016 => Self::ES2016,
      ESTarget::Es2017 => Self::ES2017,
      ESTarget::Es2018 => Self::ES2018,
      ESTarget::Es2019 => Self::ES2019,
      ESTarget::Es2020 => Self::ES2020,
      ESTarget::Es2021 => Self::ES2021,
      ESTarget::Es2022 => Self::ES2022,
      ESTarget::Es2023 => Self::ES2023,
      ESTarget::Es2024 => Self::ES2024,
      ESTarget::EsNext => Self::ESNext,
    }
  }
}
