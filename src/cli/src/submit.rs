use frontend_api::Client;
use graphql_client::GraphQLQuery;
use serde_json::{json, Value};
use std::process::exit;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Opt {
    /// problem code, e.g. "A"
    #[structopt(long, short = "p")]
    problem: String,
    #[structopt(long, short = "t")]
    toolchain: String,
    #[structopt(long, short = "f")]
    filename: String,
}

fn resolve_toolchain(client: &Client, name: &str) -> String {
    let vars = crate::queries::list_toolchains::Variables {};

    let res = client
        .query::<_, crate::queries::list_toolchains::ResponseData>(
            &crate::queries::ListToolchains::build_query(vars),
        )
        .expect("network error")
        .into_result();
    let res = res.expect("Couldn't get toolchain information");
    for tc in res.toolchains {
        if tc.id == name {
            return tc.id;
        }
    }
    panic!("Couldn't find toolchain {}", name);
}

fn resolve_problem(client: &Client, contest_name: &str, problem_code: &str) -> (String, String) {
    let data = client
        .query::<_, crate::queries::list_contests::ResponseData>(
            &crate::queries::ListContests::build_query(crate::queries::list_contests::Variables {
                detailed: true,
            }),
        )
        .expect("network error")
        .into_result()
        .expect("request rejected");
    let mut target_contest = None;
    for contest in data.contests {
        if contest.id == contest_name {
            target_contest = Some(contest);
            break;
        }
    }
    let contest = target_contest.unwrap_or_else(|| {
        eprintln!("contest {} not found", contest_name);
        exit(1);
    });

    for problem in contest.problems {
        if problem.id == problem_code {
            return (contest.id, problem.id);
        }
    }
    eprintln!("problem {} not found", problem_code);
    exit(1);
}

pub fn exec(opt: Opt, params: &super::CommonParams) -> Value {
    let data = std::fs::read(&opt.filename).expect("Couldn't read file");
    let data = base64::encode(&data);

    let tc_id = resolve_toolchain(&params.client, &opt.toolchain);
    let (_contest, problem) = resolve_problem(&params.client, "TODO", &opt.problem);

    let vars = crate::queries::submit::Variables {
        toolchain: tc_id,
        code: data,
        problem,
    };

    let resp = params
        .client
        .query::<_, crate::queries::submit::ResponseData>(&crate::queries::Submit::build_query(
            vars,
        ))
        .expect("network error")
        .into_result();
    let resp = resp.expect("submit failed");
    json!({ "id": resp })
}
