use crate::schema;
use diesel::prelude::*;

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
    pub fn from_src(src: &str, conn: &mut Conn) -> QueryResult<Option<Self>> {
        let link = schema::links::dsl::links
            .select(Link::as_select())
            .filter(schema::links::src.eq(src))
            .first::<Self>(conn)
            .optional()?;
        Ok(link)
    }

    pub fn delete(id: i32, conn: &mut Conn) -> QueryResult<()> {
        diesel::delete(schema::links::table.filter(schema::links::id.eq(id))).execute(conn)?;
        Ok(())
    }

    pub fn link(src: &str, target: &str, conn: &mut Conn) -> QueryResult<()> {
        diesel::insert_into(schema::links::table)
            .values((schema::links::src.eq(src), schema::links::target.eq(target)))
            .execute(conn)?;
        Ok(())
    }
}

pub fn new_pool(db_url: String) -> anyhow::Result<Pool> {
    let man = diesel::r2d2::ConnectionManager::new(db_url);
    let pool = Pool::builder().build(man)?;
    Ok(pool)
}
