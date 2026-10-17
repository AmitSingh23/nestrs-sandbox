use sea_orm_migration::prelude::*;

mod m20261017_000001_create_fighters_and_fights;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20261017_000001_create_fighters_and_fights::Migration)]
    }
}
