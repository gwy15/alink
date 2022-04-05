use anyhow::{Context, Result};
use std::path::Path;

pub type Pool = sqlx::SqlitePool;

pub struct Link {
    pub id: i64,
    pub src: String,
    pub target: String,
}

impl Link {
    pub async fn from_src(src: &Path, pool: &Pool) -> Result<Option<Self>> {
        let src = src.to_str().context("cannot convert path to str")?;
        let link = sqlx::query_as!(
            Self,
            r#"
            SELECT
                `id`, `src`, `target`
            FROM
                `links`
            WHERE
                `src` = ?
            LIMIT 1;
            "#,
            src
        )
        .fetch_optional(pool)
        .await?;
        Ok(link)
    }

    pub async fn delete(id: i64, pool: &Pool) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM
                `links`
            WHERE
                `id` = ?;
            "#,
            id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn link(src: &Path, target: &Path, pool: &Pool) -> Result<()> {
        let src = src.to_str().context("cannot convert path to str")?;
        let target = target.to_str().context("cannot convert path to str")?;
        sqlx::query!(
            r#"
            INSERT INTO `links`
                (`src`, `target`)
            VALUES
                (?, ?);
            "#,
            src,
            target
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}
