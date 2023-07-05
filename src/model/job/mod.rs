pub mod cmd;
pub mod cmd_group;
pub mod cmd_env;


use serde::{Serialize, Deserialize};
use threadpool::ThreadPool;
use crate::model::aliases::{Alias, Aliases};
use crate::model::job::cmd::Cmd;
use crate::model::job::cmd_env::CmdEnv;
use crate::model::job::cmd_group::CmdGroup;
use crate::model::project::Project;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Job {
    Exec(Cmd),
    Batch(CmdGroup),
}

impl Job {
    pub fn enqueue(&self, queue: &mut Vec<CmdEnv>, project: &Project, aliases: &Aliases) {
        match self {
            Job::Exec(cmd) => cmd.enqueue(queue, project.clone(), aliases.clone()),
            Job::Batch(group) => group.enqueue(queue, project, aliases)
        }
    }

    pub fn exec_on_pool(&self, pool: ThreadPool, project: &Project, aliases: &Aliases) {
        match self {
            Job::Exec(cmd) => cmd.exec_on_pool(pool, project.clone(), aliases.clone()),
            Job::Batch(group) => group.exec_on_pool(pool, project, aliases)
        }
    }
}

fn cartesian_product(foreach: &Vec<(String, Vec<Alias>)>, ctx: &mut Aliases, i: usize) -> Vec<Aliases> {
    let mut contexts = Vec::new();
    if i == foreach.len() {
        contexts.push(ctx.clone());
    } else {
        for value in &foreach[i].1 {
            ctx.insert(foreach[i].0.clone(), value.clone());
            contexts.append(&mut cartesian_product(foreach, ctx, i + 1));
        }
        ctx.remove(&foreach[i].0);
    }
    contexts
}

