use std::sync::Mutex;

use crate::repository::Repository;
use crate::db::Database;

pub struct SqliteRepository<'a> {
    db: &'a Mutex<Database>,
}

impl<'a> SqliteRepository<'a> {
    pub fn new(db: &'a Mutex<Database>) -> Self {
        SqliteRepository { db }
    }
}
impl Repository for SqliteRepository<'_> {
    fn get_scores(&self) -> Result<Vec<crate::db::ScoreRow>, String> {
        let db = self.db.lock().unwrap();
        db.list_all_scores().map_err(|e| e.to_string())
    }

    fn get_scores_today(&self) -> Result<Vec<crate::db::ScoreRow>, String> {
        todo!()
    }

    fn get_scores_user(&self, user_id: String) -> Result<Vec<crate::db::ScoreRow>, String> {
        let db = self.db.lock().unwrap();
        db.list_scores(&user_id).map_err(|e| e.to_string())
    }
}
