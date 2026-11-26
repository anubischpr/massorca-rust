use std::collections::HashMap;

use rusqlite::{Transaction, params};

use crate::bots::{BotRow, insert_bot_row};
use crate::error::StoreError;
use crate::store::Store;

impl Store {
    /// A bot's skill names, in the order they were set.
    pub fn bot_skills(&self, bot_id: &str) -> Result<Vec<String>, StoreError> {
        self.query_all(
            "SELECT skill FROM bot_skills WHERE bot_id = ?1 ORDER BY position",
            [bot_id],
            |row| row.get(0),
        )
    }

    /// The skills of every bot of a team that has any, by bot id.
    pub fn team_bot_skills(
        &self,
        team_id: &str,
    ) -> Result<HashMap<String, Vec<String>>, StoreError> {
        let rows: Vec<(String, String)> = self.query_all(
            "SELECT s.bot_id, s.skill FROM bot_skills s JOIN bots b ON b.id = s.bot_id WHERE b.team_id = ?1 ORDER BY s.bot_id, s.position",
            [team_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let mut skills: HashMap<String, Vec<String>> = HashMap::new();
        for (bot_id, skill) in rows {
            skills.entry(bot_id).or_default().push(skill);
        }
        Ok(skills)
    }

    /// Replaces a bot's skills with `skills`, in order; an empty list clears them.
    pub fn set_bot_skills(&self, bot_id: &str, skills: &[String]) -> Result<(), StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        write_skills(&transaction, bot_id, skills)?;
        transaction.commit()?;
        Ok(())
    }

    /// Inserts a bot and its skills together, like [`Store::insert_bot`].
    pub fn insert_bot_with_skills(
        &self,
        row: &BotRow,
        skills: &[String],
    ) -> Result<(), StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        insert_bot_row(&transaction, row)?;
        write_skills(&transaction, &row.id, skills)?;
        transaction.commit()?;
        Ok(())
    }
}

fn write_skills(
    transaction: &Transaction<'_>,
    bot_id: &str,
    skills: &[String],
) -> Result<(), StoreError> {
    transaction.execute("DELETE FROM bot_skills WHERE bot_id = ?1", [bot_id])?;
    for (position, skill) in skills.iter().enumerate() {
        transaction.execute(
            "INSERT INTO bot_skills (bot_id, position, skill) VALUES (?1, ?2, ?3)",
            params![bot_id, i64::try_from(position).unwrap_or(i64::MAX), skill],
        )?;
    }
    Ok(())
}
