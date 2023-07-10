pub mod cmd;
pub mod cmd_group;
pub mod cmd_env;


use eval::{Expr, to_value};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use threadpool::ThreadPool;
use crate::model::aliases::{Alias, Aliases};
use crate::model::job::cmd::Cmd;
use crate::model::job::cmd_env::CmdEnv;
use crate::model::job::cmd_group::{AliasIter, CmdGroup};
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

fn eval(expression: &String, ctx: &Aliases) -> Value {
    let mut expr = Expr::new(expression);
    for (key, value) in ctx.iter() {
        expr = match value {
            Alias::Boolean(b) => expr.value(key, b),
            Alias::Integer(i) => expr.value(key, i),
            Alias::Float(f) => expr.value(key, f),
            Alias::String(s) => expr.value(key, s)
        }
    }
    expr.exec().unwrap()
}

fn cartesian_product(foreach: &Vec<(String, AliasIter)>, ctx: &mut Aliases, i: usize, conditions: &Vec<String>) -> Vec<Aliases> {
    let mut contexts = Vec::new();
    if i == foreach.len() {
        if conditions.iter().any(|it| eval(it, ctx) == to_value(true)) {
            contexts.push(ctx.clone());
        }
    } else {
        let values = foreach[i].1.to_vec();
        for value in &values {
            ctx.insert(foreach[i].0.clone(), value.clone());
            contexts.append(&mut cartesian_product(foreach, ctx, i + 1, conditions));
        }
        ctx.remove(&foreach[i].0);
    }
    contexts
}

