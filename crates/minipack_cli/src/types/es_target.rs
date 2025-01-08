use clap::ValueEnum;

#[derive(PartialEq, Eq, Clone, ValueEnum)]
#[clap(rename_all = "lower")]
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
  EsNext,
}

impl From<ESTarget> for minipack::ESTarget {
  fn from(value: ESTarget) -> Self {
    match value {
      ESTarget::Es5 => minipack::ESTarget::Es5,
      ESTarget::Es2015 => minipack::ESTarget::Es2015,
      ESTarget::Es2016 => minipack::ESTarget::Es2016,
      ESTarget::Es2017 => minipack::ESTarget::Es2017,
      ESTarget::Es2018 => minipack::ESTarget::Es2018,
      ESTarget::Es2019 => minipack::ESTarget::Es2019,
      ESTarget::Es2020 => minipack::ESTarget::Es2020,
      ESTarget::Es2021 => minipack::ESTarget::Es2021,
      ESTarget::Es2022 => minipack::ESTarget::Es2022,
      ESTarget::Es2023 => minipack::ESTarget::Es2023,
      ESTarget::Es2024 => minipack::ESTarget::Es2024,
      ESTarget::EsNext => minipack::ESTarget::EsNext,
    }
  }
}
