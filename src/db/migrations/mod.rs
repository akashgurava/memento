pub(super) const MIGRATIONS: &[(&str, &str)] =
    &[("001_initial_schema", include_str!("001_init.sql"))];
