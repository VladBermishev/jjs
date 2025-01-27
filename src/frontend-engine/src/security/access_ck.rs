use crate::security::Token;
use snafu::Snafu;

/// Access check service
pub(crate) struct AccessChecker<'a> {
    pub(crate) token: &'a Token,
    pub(crate) cfg: &'a cfg::Config,
    pub(crate) db: &'a dyn db::DbConn,
}

#[derive(Debug, Snafu)]
pub(crate) enum AccessCheckError {
    NotFound,
    Db { source: db::Error },
}

impl From<db::Error> for AccessCheckError {
    fn from(source: db::Error) -> Self {
        Self::Db { source }
    }
}

pub(crate) type AccessResult = Result<bool, AccessCheckError>;

impl AccessChecker<'_> {
    pub(crate) fn user_can_submit(&self, contest_id: &str) -> AccessResult {
        let contest = self
            .cfg
            .find_contest(contest_id)
            .ok_or(AccessCheckError::NotFound)?;
        if self.user_is_contest_sudo(contest_id)? {
            return Ok(true);
        }
        for registered_group in &contest.group {
            if self.token.user_info.groups.contains(registered_group) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn is_sudo(&self) -> AccessResult {
        // When namespaces are introduced, this function will account for that
        Ok(self.token.user_info.name == "Global/Root")
    }

    fn user_is_contest_sudo(&self, contest_id: &str) -> AccessResult {
        if self.is_sudo()? {
            return Ok(true);
        }
        let contest = self
            .cfg
            .find_contest(contest_id)
            .ok_or(AccessCheckError::NotFound)?;
        for judges_group in &contest.judges {
            if self.token.user_info.groups.contains(judges_group) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn user_can_modify_run(&self, run_id: i32) -> AccessResult {
        if self.user_is_contest_sudo(
            // TODO: correctly determine contest_id here
            "TODO",
        )? {
            return Ok(true);
        }
        let run = self.db.run_load(run_id)?;

        Ok(run.user_id == self.token.user_id())
    }
}
