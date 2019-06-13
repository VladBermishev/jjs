use super::access_control::{ContestRights, GlobalRights};
use acl::{AccessToken, Prefix};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub groups: Vec<String>,
}

impl UserInfo {
    pub(crate) fn retrieve(name: &str, conn: &PgConnection) -> UserInfo {
        use db::schema::{users::dsl::*, User};
        let user_data: User = users
            .filter(username.eq(name))
            .load(conn)
            .expect("db error")
            .into_iter()
            .nth(0)
            .unwrap();

        UserInfo {
            name: user_data.username,
            groups: user_data.groups,
        }
    }

    fn as_access_token<'a>(&'a self) -> AccessToken<'a> {
        AccessToken {
            name: &self.name,
            groups: &self.groups,
        }
    }

    pub fn for_user_and_groups() {}
}

pub struct AccessChecker<'a> {
    pub root: &'a Prefix,
    pub user_info: &'a UserInfo,
}

impl<'a> AccessChecker<'a> {
    fn check(&self, path: &[&str], access: u64) -> bool {
        acl::access(self.root, self.user_info.as_access_token(), path, access).ok() == Some(access)
    }

    pub fn can_submit(&self) -> bool {
        let path = &["Contest", "CommonRights"];
        let desired_access = ContestRights::SUBMIT;
        self.check(path, desired_access.bits())
    }

    pub fn can_create_users(&self) -> bool {
        self.check(&["CommonRights"], (GlobalRights::MANAGE_USERS).bits())
    }

    pub fn can_view_contest(&self) -> bool {
        self.check(&["Contest", "CommonRights"], (ContestRights::VIEW).bits())
    }
}
