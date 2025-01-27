mod contest;
mod run;

use juniper::{GraphQLInputObject, GraphQLObject};
use uuid::Uuid;

pub(crate) use contest::{Contest, Problem};
pub(crate) use run::Run;

pub type ToolchainId = String;
pub type RunId = i32;
pub type ProblemId = String;
pub type ContestId = String;
pub type UserId = Uuid;

#[derive(GraphQLInputObject)]
pub(crate) struct InvokeStatusIn {
    pub kind: String,
    pub code: String,
}

#[derive(GraphQLObject)]
pub(crate) struct InvokeStatusOut {
    pub kind: String,
    pub code: String,
}

#[derive(GraphQLObject)]
pub(crate) struct User {
    pub id: UserId,
    pub login: String,
}

impl<'a> From<&'a db::schema::User> for User {
    fn from(user: &'a db::schema::User) -> User {
        User {
            id: user.id,
            login: user.username.clone(),
        }
    }
}

#[derive(GraphQLObject)]
pub(crate) struct Toolchain {
    /// Human readable name, e.g. "GCC C++ v9.1 with sanitizers enables"
    pub name: String,
    /// Internal name, e.g. "cpp.san.9.1"
    pub id: ToolchainId,
}

impl<'a> From<&'a cfg::Toolchain> for Toolchain {
    fn from(tc: &'a cfg::Toolchain) -> Self {
        Self {
            name: tc.title.clone(),
            id: tc.name.clone(),
        }
    }
}

#[derive(GraphQLObject)]
/// Type that represents session
/// You shouldn't do any assumptions about this type representation
pub(crate) struct SessionToken {
    /// Opaque string that represents session data
    /// On all subsequent requests, that this string as value of header `X-Jjs-Auth`
    pub data: String,

    /// in dev mode, contains session data in unencrypted form
    pub raw_data: Option<String>,
}
