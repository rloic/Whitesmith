use std::time::Duration;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub enum Job {
    Exec(Cmd),
    Batch(CmdGroup),
}

impl Job {
    pub fn to_cmds(&self, ctx: &HashMap<String, String>) -> Vec<(&Cmd, HashMap<String, String>)> {
        match self {
            Job::Exec(cmd) => vec![(cmd, ctx.clone())],
            Job::Batch(cmd_group) => {
                let mut tuples = Vec::with_capacity(cmd_group.foreach.len());
                for (key, values) in cmd_group.foreach.iter() {
                    tuples.push((key.clone(), values.clone()))
                }
                let contexts = cartesian_product(&tuples, &mut ctx.clone(), 0);

                cmd_group.run.iter()
                    .flat_map(|it| contexts.iter().flat_map(|ctx| (it.to_cmds(ctx))).collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            }
        }
    }
}

fn cartesian_product(foreach: &Vec<(String, Vec<String>)>, ctx: &mut HashMap<String, String>, i: usize) -> Vec<HashMap<String, String>> {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Cmd {
    pub name: String,
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default)]
    pub order: u32,
    #[serde(default, with = "humantime_serde")]
    pub timeout: Option<Duration>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CmdGroup {
    pub foreach: HashMap<String, Vec<String>>,
    pub run: Vec<Job>,
    #[serde(default)]
    pub order: u32,
}