use crate::schema;
use anyhow::{Context, Result};
use diesel::prelude::*;
use std::path::Path;

pub type Conn = diesel::SqliteConnection;
pub type Pool = diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<Conn>>;

#[derive(Queryable, PartialEq, Debug, Selectable)]
#[diesel(table_name = schema::links)]
pub struct Link {
    pub id: i32,
    pub src: String,
    pub target: String,
}

impl Link {
    pub fn from_src(src: &Path, conn: &mut Conn) -> Result<Option<Self>> {
        let src_str = src.to_str().context("cannot convert path to str")?;
        let link = schema::links::dsl::links
            .select(Link::as_select())
            .filter(schema::links::src.eq(src_str))
            .first::<Self>(conn)
            .optional()?;
        Ok(link)
    }

    pub fn delete(id: i32, conn: &mut Conn) -> Result<()> {
        diesel::delete(schema::links::table.filter(schema::links::id.eq(id))).execute(conn)?;
        Ok(())
    }

    pub fn link(src: &Path, target: &Path, conn: &mut Conn) -> Result<()> {
        let src = src.to_str().context("cannot convert path to str")?;
        let target = target.to_str().context("cannot convert path to str")?;
        diesel::insert_into(schema::links::table)
            .values((schema::links::src.eq(src), schema::links::target.eq(target)))
            .execute(conn)?;
        Ok(())
    }
}
