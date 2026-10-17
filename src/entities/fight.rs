use sea_orm::entity::prelude::*;

/// How sure the reporting is that the bout will happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum FightStatus {
    #[sea_orm(string_value = "confirmed")]
    Confirmed,
    #[sea_orm(string_value = "rumored")]
    Rumored,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "fights")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub fighter_a_id: i32,
    pub fighter_b_id: i32,
    pub status: FightStatus,
    pub event_name: Option<String>,
    /// `None` when the report doesn't give a date yet (common for rumors).
    pub event_date: Option<Date>,
    pub source_name: String,
    pub source_url: String,
    pub headline: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::fighter::Entity",
        from = "Column::FighterAId",
        to = "super::fighter::Column::Id"
    )]
    FighterA,
    #[sea_orm(
        belongs_to = "super::fighter::Entity",
        from = "Column::FighterBId",
        to = "super::fighter::Column::Id"
    )]
    FighterB,
}

impl ActiveModelBehavior for ActiveModel {}
