use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Fighters::Table)
                    .col(
                        ColumnDef::new(Fighters::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Fighters::Name)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Fighters::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Fights::Table)
                    .col(
                        ColumnDef::new(Fights::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Fights::FighterAId).integer().not_null())
                    .col(ColumnDef::new(Fights::FighterBId).integer().not_null())
                    .col(ColumnDef::new(Fights::Status).string_len(16).not_null())
                    .col(ColumnDef::new(Fights::EventName).string())
                    .col(ColumnDef::new(Fights::EventDate).date())
                    .col(ColumnDef::new(Fights::SourceName).string().not_null())
                    .col(ColumnDef::new(Fights::SourceUrl).text().not_null())
                    .col(ColumnDef::new(Fights::Headline).text().not_null())
                    .col(
                        ColumnDef::new(Fights::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Fights::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .check(Expr::col(Fights::Status).is_in(["confirmed", "rumored"]))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Fights::Table, Fights::FighterAId)
                            .to(Fighters::Table, Fighters::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Fights::Table, Fights::FighterBId)
                            .to(Fighters::Table, Fighters::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_fights_fighters")
                    .table(Fights::Table)
                    .col(Fights::FighterAId)
                    .col(Fights::FighterBId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Fights::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Fighters::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Fighters {
    Table,
    Id,
    Name,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Fights {
    Table,
    Id,
    FighterAId,
    FighterBId,
    Status,
    EventName,
    EventDate,
    SourceName,
    SourceUrl,
    Headline,
    CreatedAt,
    UpdatedAt,
}
