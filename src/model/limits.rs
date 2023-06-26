use std::io;
use std::time::Duration;
use bytesize::{ByteSize};
use rlimit::Resource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Limits {
    #[serde(default, with = "humantime_serde")]
    pub cpu_time: Option<Duration>,
    #[serde(default)]
    pub file_size: Option<ByteSize>,
    #[serde(default)]
    pub data_size: Option<ByteSize>,
    #[serde(default)]
    pub stack_size: Option<ByteSize>,
    #[serde(default)]
    pub core_file_size: Option<ByteSize>,
    #[serde(default)]
    pub processes: Option<u64>,
    #[serde(default)]
    pub open_files: Option<u64>,
    #[serde(default)]
    pub locked_memory: Option<ByteSize>,
    #[serde(default)]
    pub address_space: Option<ByteSize>,
    #[serde(default)]
    pub file_locks: Option<u64>,
    #[serde(default)]
    pub pending_signals: Option<u64>,
    #[serde(default)]
    pub msgqueue_size: Option<ByteSize>,
    #[serde(default)]
    pub nice_priority: Option<u64>,
    #[serde(default)]
    pub realtime_priority: Option<u64>,
    #[serde(default, with = "humantime_serde")]
    pub realtime_timeout: Option<Duration>,
}

impl Limits {
    pub fn apply(&self) -> io::Result<()> {
        if let Some(cpu_time) = self.cpu_time {
            Resource::CPU.set(cpu_time.as_secs(), cpu_time.as_secs())?;
        }

        if let Some(file_size) = self.file_size {
            Resource::FSIZE.set(file_size.0, file_size.0)?;
        }

        if let Some(data_size) = self.data_size {
            Resource::DATA.set(data_size.0, data_size.0)?;
        }

        if let Some(stack_size) = self.stack_size {
            Resource::STACK.set(stack_size.0, stack_size.0)?;
        }

        if let Some(core_file_size) = self.core_file_size {
            Resource::CORE.set(core_file_size.0, core_file_size.0)?;
        }

        if let Some(processes) = self.processes {
            Resource::NPROC.set(processes, processes)?;
        }

        if let Some(open_files) = self.open_files {
            Resource::NOFILE.set(open_files, open_files)?;
        }

        if let Some(locked_memory) = self.locked_memory {
            Resource::MEMLOCK.set(locked_memory.0, locked_memory.0)?;
        }

        if let Some(address_space) = self.address_space {
            Resource::AS.set(address_space.0, address_space.0)?;
        }

        if let Some(file_locks) = self.file_locks {
            Resource::LOCKS.set(file_locks, file_locks)?;
        }

        if let Some(pending_signals) = self.pending_signals {
            Resource::SIGPENDING.set(pending_signals, pending_signals)?;
        }

        if let Some(msgqueue_size) = self.msgqueue_size {
            Resource::MSGQUEUE.set(msgqueue_size.0, msgqueue_size.0)?;
        }

        if let Some(nice_priority) = self.nice_priority {
            Resource::NICE.set(nice_priority, nice_priority)?;
        }

        if let Some(realtime_priority) = self.realtime_priority {
            Resource::RTPRIO.set(realtime_priority, realtime_priority)?;
        }

        if let Some(realtime_timeout) = self.realtime_timeout {
            Resource::RTTIME.set(realtime_timeout.as_secs(), realtime_timeout.as_secs())?;
        }

        Ok(())
    }
}

