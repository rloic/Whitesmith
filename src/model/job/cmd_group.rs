use std::collections::HashMap;
use crate::model::aliases::{Alias, Aliases};
use crate::model::job::{cartesian_product, Job};
use crate::model::project::Project;
use serde::{Serialize, Deserialize};
use threadpool::ThreadPool;
use crate::model::job::cmd_env::CmdEnv;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CmdGroup {
    pub foreach: HashMap<String, AliasIter>,
    #[serde(rename="where", default)]
    pub conditions: Vec<String>,
    pub apply: Batch,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum AliasIter {
    Vec(Vec<Alias>),
    ClosedIntRange(ClosedIntRange)
}

impl AliasIter {
    pub(crate) fn to_vec(&self) -> Vec<Alias> {
        match self {
            AliasIter::Vec(vec) => vec.clone(),
            AliasIter::ClosedIntRange(range) => (range.start..=range.end_inclusive).map(|it| Alias::Integer(it)).collect::<Vec<_>>()
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClosedIntRange {
    start: i64,
    end_inclusive: i64
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Batch {
    pub aliases: Aliases,
    pub cmds: Vec<Job>,
}

impl CmdGroup {
    fn generate_context_combinations(&self, aliases: &Aliases) -> Vec<Aliases> {
        let mut tuples = Vec::with_capacity(self.foreach.len());
        for (key, values) in self.foreach.iter() {
            tuples.push((key.clone(), values.clone()))
        }
        let mut current_aliases = aliases.clone();
        for (key, values) in self.apply.aliases.iter() {
            current_aliases.insert(key.clone(), values.clone());
        }
        cartesian_product(&tuples, &mut current_aliases, 0, &self.conditions)
    }

    pub(crate) fn enqueue(&self, queue: &mut Vec<CmdEnv>, project: &Project, aliases: &Aliases) {
        for context in &self.generate_context_combinations(aliases) {
            for job in &self.apply.cmds {
                job.enqueue(queue, project, context);
            }
        }
    }

    pub(crate) fn exec_on_pool(&self, pool: ThreadPool, project: &Project, parent_aliases: &Aliases) {
        let cmds = &self.apply.cmds;
        for captured_context in self.generate_context_combinations(parent_aliases) {
            let captured_pool = pool.clone();
            let captured_jobs = cmds.clone();
            let captured_project = project.clone();
            pool.execute(move || {
                for job in captured_jobs {
                    let inner_pool = captured_pool.clone();
                    //let inner_project = captured_project.clone();
                    //let inner_context = captured_context.clone();
                    job.exec_on_pool(inner_pool, &captured_project, &captured_context);
                }
            })
        }
    }
}